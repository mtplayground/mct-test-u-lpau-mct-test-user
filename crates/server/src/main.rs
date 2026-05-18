mod config;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::{extract::Request, http::StatusCode, response::IntoResponse, routing::get, Router};
use tokio::{net::TcpListener, signal};
use tower_http::trace::TraceLayer;
use tracing::{info, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::config::{Config, ConfigError};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    init_tracing()?;

    let config = Config::from_env().map_err(AppError::Config)?;
    let app = build_router();
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), config.port);
    let listener = TcpListener::bind(addr).await.map_err(AppError::Bind)?;

    info!(
        address = %addr,
        chromium_path = %config.chromium_path.display(),
        scan_timeout_secs = config.scan_timeout.as_secs(),
        database_configured = !config.database_url.is_empty(),
        anthropic_key_configured = !config.anthropic_api_key.is_empty(),
        "server listening"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(AppError::Serve)
}

fn build_router() -> Router {
    Router::new()
        .route("/healthz", get(healthz))
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

async fn healthz() -> impl IntoResponse {
    StatusCode::OK
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
    Serve(std::io::Error),
    Tracing(tracing_subscriber::util::TryInitError),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bind(error) => write!(f, "failed to bind TCP listener: {error}"),
            Self::Config(error) => write!(f, "{error}"),
            Self::Serve(error) => write!(f, "server exited with error: {error}"),
            Self::Tracing(error) => write!(f, "failed to initialize tracing: {error}"),
        }
    }
}

impl std::error::Error for AppError {}
