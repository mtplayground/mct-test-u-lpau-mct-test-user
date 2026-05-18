mod config;

use std::{net::{IpAddr, Ipv4Addr, SocketAddr}, str::FromStr, sync::Arc};

use axum::{
    extract::{Path, Request, State},
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
    category_breakdown, validate_scan_url, Category, Finding, FindingKind, NewScan, Scan,
    ScanPhase, ScanStatus, UrlValidationError,
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
        .route("/scans/{id}", get(get_scan))
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

async fn get_scan(
    State(state): State<AppState>,
    Path(scan_id): Path<i64>,
) -> Result<Json<GetScanResponse>, ApiError> {
    let scan = state
        .repository
        .find_scan_by_id(scan_id)
        .await
        .map_err(ApiError::Repository)?
        .ok_or(ApiError::NotFound)?;

    let findings = state
        .repository
        .list_findings_for_scan(scan.id)
        .await
        .map_err(ApiError::Repository)?;

    Ok(Json(build_scan_response(scan, findings)))
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
struct GetScanResponse {
    id: i64,
    status: String,
    phase: String,
    accessibility_score: Option<i32>,
    inappropriate_score: Option<i32>,
    risk_level: Option<String>,
    error_reason: Option<String>,
    accessibility: Vec<FindingResponse>,
    inappropriate: Vec<FindingResponse>,
    category_breakdown: Option<Vec<CategoryBreakdownItem>>,
}

#[derive(Debug, Serialize)]
struct FindingResponse {
    id: i64,
    title: String,
    category: String,
    severity: String,
    summary: String,
    location: Option<String>,
    suggestion: Option<String>,
    example_excerpt: Option<String>,
    why_unsafe: Option<String>,
}

#[derive(Debug, Serialize)]
struct CategoryBreakdownItem {
    category: String,
    count: usize,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug)]
enum ApiError {
    NotFound,
    Repository(RepositoryError),
    Validation(ValidationError),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Scan not found.".to_owned(),
                }),
            )
                .into_response(),
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

fn build_scan_response(scan: Scan, findings: Vec<Finding>) -> GetScanResponse {
    let mut accessibility = Vec::new();
    let mut inappropriate = Vec::new();

    for finding in findings {
        let response = FindingResponse {
            id: finding.id,
            title: finding.title,
            category: finding.category.as_str().to_owned(),
            severity: finding.severity.as_str().to_owned(),
            summary: finding.summary,
            location: finding.location,
            suggestion: finding.suggestion,
            example_excerpt: finding.example_excerpt,
            why_unsafe: finding.why_unsafe,
        };

        match finding.kind {
            FindingKind::Accessibility => accessibility.push(response),
            FindingKind::ContentSafety => inappropriate.push(response),
        }
    }

    let category_breakdown = if scan.status == ScanStatus::Completed {
        let mut categories = accessibility
            .iter()
            .map(|_| Category::Accessibility)
            .collect::<Vec<_>>();
        categories.extend(
            inappropriate
                .iter()
                .filter_map(|finding| Category::from_str(&finding.category).ok()),
        );

        Some(
            category_breakdown(categories)
                .into_iter()
                .map(|(category, count)| CategoryBreakdownItem {
                    category: category.as_str().to_owned(),
                    count,
                })
                .collect(),
        )
    } else {
        None
    };

    GetScanResponse {
        id: scan.id,
        status: scan.status.as_str().to_owned(),
        phase: scan.phase.as_str().to_owned(),
        accessibility_score: scan.accessibility_score,
        inappropriate_score: scan.inappropriate_score,
        risk_level: scan.risk_level.map(|level| level.as_str().to_owned()),
        error_reason: scan.error_reason,
        accessibility,
        inappropriate,
        category_breakdown,
    }
}
