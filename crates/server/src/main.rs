mod config;

use std::{net::{IpAddr, Ipv4Addr, SocketAddr}, sync::Arc};

use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use tokio::{net::TcpListener, signal};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing::{info, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use zeroclaw_core::{
    validate_scan_url, NewScan, ScanPhase, ScanStatus, UrlValidationError,
};
use zeroclaw_storage::{Database, DatabaseError, Repository, RepositoryError};

use crate::config::{Config, ConfigError};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    init_tracing()?;

    let config = Config::from_env().map_err(AppError::Config)?;
    let database = Database::connect(&config.database_url)
        .await
        .map_err(AppError::Database)?;
    let app = build_router(AppState {
        repository: Arc::new(Repository::new(database.pool().clone())),
    });
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), config.port);
    let listener = TcpListener::bind(addr).await.map_err(AppError::Bind)?;

    info!(
        address = %addr,
        database_connections = database.pool().size(),
        anthropic_key_configured = !config.anthropic_api_key.is_empty(),
        chromium_path = %config.chromium_path.display(),
        scan_timeout_secs = config.scan_timeout.as_secs(),
        migrations_ran = true,
        "server listening"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(AppError::Serve)
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .nest("/api", api_router())
        .fallback_service(spa_service())
        .with_state(state)
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                tracing::span!(
                    Level::INFO,
                    "http_request",
                    method = %request.method(),
                    uri = %request.uri()
                )
            }),
        )
}

fn api_router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/scans", post(create_scan))
        .fallback(api_not_found)
}

fn spa_service() -> ServeDir<ServeFile> {
    ServeDir::new("web/dist").fallback(ServeFile::new("web/dist/index.html"))
}

async fn healthz() -> impl IntoResponse {
    StatusCode::OK
}

async fn api_not_found() -> impl IntoResponse {
    StatusCode::NOT_FOUND
}

async fn create_scan(
    State(state): State<AppState>,
    Json(request): Json<CreateScanRequest>,
) -> Result<Json<CreateScanResponse>, ApiError> {
    let validated = validate_scan_url(&request.url)
        .map_err(|source| ApiError::Validation(ValidationError::new(source)))?;
    let force = request.force.unwrap_or(false);

    if !force {
        if let Some(scan) = state
            .repository
            .find_recent_completed_by_url(&validated.normalized_url)
            .await
            .map_err(ApiError::Repository)?
        {
            let age = OffsetDateTime::now_utc() - scan.updated_at;
            if age <= Duration::hours(24) {
                return Ok(Json(CreateScanResponse {
                    id: scan.id,
                    cached: true,
                }));
            }
        }
    }

    let scan = state
        .repository
        .insert_scan(&NewScan {
            url: request.url,
            normalized_url: validated.normalized_url,
            status: ScanStatus::Pending,
            phase: ScanPhase::Queued,
        })
        .await
        .map_err(ApiError::Repository)?;

    spawn_worker_stub(scan.id);

    Ok(Json(CreateScanResponse {
        id: scan.id,
        cached: false,
    }))
}

fn spawn_worker_stub(scan_id: i64) {
    tokio::spawn(async move {
        tracing::info!(scan_id, "worker stub spawned for scan");
    });
}

fn init_tracing() -> Result<(), AppError> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,tower_http=info"));

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(env_filter)
        .try_init()
        .map_err(AppError::Tracing)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = signal::ctrl_c().await {
            tracing::warn!(error = %error, "failed to install Ctrl+C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::warn!(error = %error, "failed to install SIGTERM handler");
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("shutdown signal received");
}

#[derive(Debug)]
enum AppError {
    Bind(std::io::Error),
    Config(ConfigError),
    Database(DatabaseError),
    Serve(std::io::Error),
    Tracing(tracing_subscriber::util::TryInitError),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bind(error) => write!(f, "failed to bind TCP listener: {error}"),
            Self::Config(error) => write!(f, "{error}"),
            Self::Database(error) => write!(f, "{error}"),
            Self::Serve(error) => write!(f, "server exited with error: {error}"),
            Self::Tracing(error) => write!(f, "failed to initialize tracing: {error}"),
        }
    }
}

impl std::error::Error for AppError {}

#[derive(Clone)]
struct AppState {
    repository: Arc<Repository>,
}

#[derive(Debug, Deserialize)]
struct CreateScanRequest {
    url: String,
    force: Option<bool>,
}

#[derive(Debug, Serialize)]
struct CreateScanResponse {
    id: i64,
    cached: bool,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug)]
enum ApiError {
    Repository(RepositoryError),
    Validation(ValidationError),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::Validation(error) => (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: error.friendly_message().to_owned(),
                }),
            )
                .into_response(),
            Self::Repository(error) => {
                tracing::error!(error = %error, "request failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Something went wrong while creating the scan.".to_owned(),
                    }),
                )
                    .into_response()
            }
        }
    }
}

#[derive(Debug)]
struct ValidationError {
    source: UrlValidationError,
}

impl ValidationError {
    fn new(source: UrlValidationError) -> Self {
        Self { source }
    }

    fn friendly_message(&self) -> &'static str {
        "Please enter a valid public http:// or https:// URL."
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.friendly_message(), self.source)
    }
}

impl std::error::Error for ValidationError {}
