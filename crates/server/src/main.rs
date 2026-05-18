use std::{env, net::{IpAddr, Ipv4Addr, SocketAddr}, num::ParseIntError};

use axum::{extract::Request, http::StatusCode, response::IntoResponse, routing::get, Router};
use tokio::{net::TcpListener, signal};
use tower_http::trace::TraceLayer;
use tracing::{info, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    init_tracing()?;

    let port = read_port()?;
    let app = build_router();
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);
    let listener = TcpListener::bind(addr).await.map_err(AppError::Bind)?;

    info!(address = %addr, "server listening");

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

fn read_port() -> Result<u16, AppError> {
    match env::var("PORT") {
        Ok(raw) => raw.parse::<u16>().map_err(|source| AppError::InvalidPort {
            value: raw,
            source,
        }),
        Err(env::VarError::NotPresent) => Ok(8080),
        Err(source) => Err(AppError::PortEnv(source)),
    }
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
    InvalidPort {
        value: String,
        source: ParseIntError,
    },
    PortEnv(env::VarError),
    Serve(std::io::Error),
    Tracing(tracing_subscriber::util::TryInitError),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bind(error) => write!(f, "failed to bind TCP listener: {error}"),
            Self::InvalidPort { value, source } => {
                write!(f, "invalid PORT value '{value}': {source}")
            }
            Self::PortEnv(error) => write!(f, "failed to read PORT from environment: {error}"),
            Self::Serve(error) => write!(f, "server exited with error: {error}"),
            Self::Tracing(error) => write!(f, "failed to initialize tracing: {error}"),
        }
    }
}

impl std::error::Error for AppError {}
