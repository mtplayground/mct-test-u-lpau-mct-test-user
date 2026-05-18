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
use zeroclaw_ai::{AnthropicClient, AnthropicClientConfig};
use zeroclaw_core::{
    category_breakdown, recommendations_text, validate_scan_url, Category, Finding, FindingKind, NewScan, Scan,
    ScanPhase, ScanStatus, UrlValidationError,
};
use zeroclaw_storage::{Database, DatabaseError, Repository, RepositoryError};
use zeroclaw_worker::{
    ChromiumPageAnalyzer, ContentSafetyClient, PageAnalyzer, ScanWorker, WorkerConfig,
};

use crate::config::{Config, ConfigError};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    init_tracing()?;

    let config = Config::from_env().map_err(AppError::Config)?;
    let database = Database::connect(&config.database_url)
        .await
        .map_err(AppError::Database)?;
    let repository = Arc::new(Repository::new(database.pool().clone()));
    let worker_config = WorkerConfig {
        chromium_path: config.chromium_path.clone(),
        scan_timeout: config.scan_timeout,
    };
    let app = build_router(AppState {
        repository: repository.clone(),
        worker_dispatcher: Arc::new(SpawnedScanWorkerDispatcher::new(
            Arc::new(ChromiumPageAnalyzer),
            Arc::new(AnthropicClient::new(AnthropicClientConfig::new(
                config.anthropic_api_key.clone(),
            ))),
            worker_config,
        )),
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

    state
        .worker_dispatcher
        .dispatch(scan.id, state.repository.clone());

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
    worker_dispatcher: Arc<dyn WorkerDispatcher>,
}

#[derive(Debug, Deserialize)]
struct CreateScanRequest {
    url: String,
    force: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CreateScanResponse {
    id: i64,
    cached: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct GetScanResponse {
    id: i64,
    url: String,
    created_at: String,
    status: String,
    phase: String,
    accessibility_score: Option<i32>,
    inappropriate_score: Option<i32>,
    risk_level: Option<String>,
    error_reason: Option<String>,
    accessibility: Vec<FindingResponse>,
    inappropriate: Vec<FindingResponse>,
    category_breakdown: Option<Vec<CategoryBreakdownItem>>,
    recommended_actions: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
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

#[derive(Debug, Deserialize, Serialize)]
struct CategoryBreakdownItem {
    category: String,
    count: usize,
}

#[derive(Debug, Deserialize, Serialize)]
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

trait WorkerDispatcher: Send + Sync {
    fn dispatch(&self, scan_id: i64, repository: Arc<Repository>);
}

struct SpawnedScanWorkerDispatcher<P, C> {
    page_analyzer: Arc<P>,
    content_safety_client: Arc<C>,
    worker_config: WorkerConfig,
}

impl<P, C> SpawnedScanWorkerDispatcher<P, C> {
    fn new(
        page_analyzer: Arc<P>,
        content_safety_client: Arc<C>,
        worker_config: WorkerConfig,
    ) -> Self {
        Self {
            page_analyzer,
            content_safety_client,
            worker_config,
        }
    }
}

impl<P, C> WorkerDispatcher for SpawnedScanWorkerDispatcher<P, C>
where
    P: PageAnalyzer + 'static,
    C: ContentSafetyClient + 'static,
{
    fn dispatch(&self, scan_id: i64, repository: Arc<Repository>) {
        let worker = ScanWorker::new(
            repository,
            self.page_analyzer.clone(),
            self.content_safety_client.clone(),
            self.worker_config.clone(),
        );

        tokio::spawn(async move {
            if let Err(error) = worker.run_scan(scan_id).await {
                tracing::error!(scan_id, error = %error, "scan worker failed");
            }
        });
    }
}

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

    let recommended_actions = category_breakdown
        .as_ref()
        .map(|breakdown: &Vec<CategoryBreakdownItem>| {
        let recommendations = recommendations_text(
            &breakdown
                .iter()
                .filter_map(|item| Category::from_str(&item.category).ok().map(|category| (category, item.count)))
                .collect(),
        );

        recommendations
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
        });

    GetScanResponse {
        id: scan.id,
        url: scan.url,
        created_at: scan.created_at.to_string(),
        status: scan.status.as_str().to_owned(),
        phase: scan.phase.as_str().to_owned(),
        accessibility_score: scan.accessibility_score,
        inappropriate_score: scan.inappropriate_score,
        risk_level: scan.risk_level.map(|level| level.as_str().to_owned()),
        error_reason: scan.error_reason,
        accessibility,
        inappropriate,
        category_breakdown,
        recommended_actions,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        net::TcpListener,
        path::{Path, PathBuf},
        process::Command,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use axum::{http::StatusCode, routing::get, Router as AxumRouter};
    use reqwest::Client;
    use serial_test::serial;
    use sqlx::{postgres::PgPoolOptions, Executor};
    use time::OffsetDateTime;
    use tokio::{net::TcpListener as TokioTcpListener, task::JoinHandle, time::sleep};
    use zeroclaw_ai::AnthropicClientError;
    use zeroclaw_browser::{AxeImpact, AxeNode, AxeViolation, BrowserSessionError};
    use zeroclaw_core::{RiskLevel, Severity};
    use zeroclaw_storage::{migrate, Repository};
    use zeroclaw_worker::{
        ContentSafetyClient as WorkerContentSafetyClient, PageAnalysis,
        PageAnalyzer as WorkerPageAnalyzer, WorkerConfig,
    };

    use super::{
        build_router, build_scan_response, AppState, Category, CreateScanResponse,
        ErrorResponse, Finding, FindingKind, GetScanResponse, Scan, ScanPhase, ScanStatus,
        SpawnedScanWorkerDispatcher, WorkerDispatcher,
    };

    const PG_BIN_DIR: &str = "/usr/lib/postgresql/16/bin";

    #[tokio::test]
    #[serial]
    async fn post_then_poll_happy_path_returns_completed_scan_results() -> Result<(), Box<dyn Error>> {
        let fixture = FixtureHtmlServer::spawn().await?;
        let worker_dispatcher = Arc::new(SpawnedScanWorkerDispatcher::new(
            Arc::new(FixturePageAnalyzer::new(fixture.base_url.clone())),
            Arc::new(StaticContentSafetyClient::new(
                r#"{
                    "summary":"Fixture page contains weapon marketing content.",
                    "findings":[
                        {
                            "title":"Weapon promotion",
                            "category":"weapons",
                            "severity":"high",
                            "summary":"The page promotes weapon purchases.",
                            "example_excerpt":"Buy tactical rifles today.",
                            "why_unsafe":"It encourages acquiring weapons.",
                            "recommended_action":"Remove direct purchase language."
                        }
                    ]
                }"#,
            )),
            WorkerConfig {
                chromium_path: PathBuf::from("/usr/bin/chromium"),
                scan_timeout: std::time::Duration::from_secs(30),
            },
        ));
        let test_app = TestApp::spawn(worker_dispatcher).await?;
        let client = Client::new();

        let create_response = client
            .post(test_app.url("/api/scans"))
            .json(&serde_json::json!({
                "url": "https://example.com",
            }))
            .send()
            .await?;

        assert_eq!(create_response.status(), StatusCode::OK);
        let created: CreateScanResponse = create_response.json().await?;
        assert!(!created.cached);

        let polled = test_app.poll_scan(&client, created.id).await?;

        assert_eq!(polled.status, "completed");
        assert_eq!(polled.phase, "completed");
        assert_eq!(polled.accessibility_score, Some(1));
        assert_eq!(polled.inappropriate_score, Some(8));
        assert_eq!(polled.risk_level.as_deref(), Some("high"));
        assert_eq!(polled.accessibility.len(), 1);
        assert_eq!(polled.inappropriate.len(), 1);
        assert_eq!(
            polled.accessibility[0].title,
            "Images must have alternative text"
        );
        assert_eq!(polled.inappropriate[0].title, "Weapon promotion");

        let breakdown = polled
            .category_breakdown
            .ok_or("completed scan should include category breakdown")?;
        assert!(breakdown.iter().any(|item| item.category == "accessibility" && item.count == 1));
        assert!(breakdown.iter().any(|item| item.category == "weapons" && item.count == 1));

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn post_scan_rejects_invalid_url() -> Result<(), Box<dyn Error>> {
        let test_app = TestApp::spawn(Arc::new(NoopWorker)).await?;
        let client = Client::new();

        let response = client
            .post(test_app.url("/api/scans"))
            .json(&serde_json::json!({
                "url": "http://127.0.0.1/private",
            }))
            .send()
            .await?;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: ErrorResponse = response.json().await?;
        assert_eq!(error.error, "Please enter a valid public http:// or https:// URL.");

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn get_scan_returns_not_found_for_unknown_id() -> Result<(), Box<dyn Error>> {
        let test_app = TestApp::spawn(Arc::new(NoopWorker)).await?;
        let client = Client::new();

        let response = client.get(test_app.url("/api/scans/999999")).send().await?;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let error: ErrorResponse = response.json().await?;
        assert_eq!(error.error, "Scan not found.");

        Ok(())
    }

    #[test]
    fn build_scan_response_splits_findings_and_includes_breakdown_for_completed_scan() {
        let response = build_scan_response(
            sample_scan(ScanStatus::Completed, ScanPhase::Completed),
            vec![
                sample_finding(
                    11,
                    FindingKind::Accessibility,
                    Category::Accessibility,
                    "Missing alt text",
                ),
                sample_finding(
                    12,
                    FindingKind::ContentSafety,
                    Category::Weapons,
                    "Weapon promotion",
                ),
            ],
        );

        assert_eq!(response.status, "completed");
        assert_eq!(response.accessibility.len(), 1);
        assert_eq!(response.inappropriate.len(), 1);

        let breakdown = response
            .category_breakdown
            .expect("completed scans should include category breakdown");

        assert!(breakdown.iter().any(|item| item.category == "accessibility" && item.count == 1));
        assert!(breakdown.iter().any(|item| item.category == "weapons" && item.count == 1));
    }

    #[test]
    fn build_scan_response_omits_breakdown_for_non_completed_scan() {
        let response = build_scan_response(
            sample_scan(ScanStatus::Running, ScanPhase::Accessibility),
            vec![sample_finding(
                21,
                FindingKind::Accessibility,
                Category::Accessibility,
                "Pending issue",
            )],
        );

        assert_eq!(response.status, "running");
        assert!(response.category_breakdown.is_none());
    }

    fn sample_scan(status: ScanStatus, phase: ScanPhase) -> Scan {
        Scan {
            id: 7,
            url: "https://example.com".to_owned(),
            normalized_url: "https://example.com/".to_owned(),
            status,
            phase,
            accessibility_score: Some(2),
            inappropriate_score: Some(4),
            risk_level: Some(RiskLevel::Medium),
            error_reason: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn sample_finding(id: i64, kind: FindingKind, category: Category, title: &str) -> Finding {
        Finding {
            id,
            scan_id: 7,
            kind,
            title: title.to_owned(),
            category,
            severity: Severity::Medium,
            summary: "summary".to_owned(),
            location: None,
            suggestion: None,
            example_excerpt: None,
            why_unsafe: None,
        }
    }

    struct TestApp {
        _admin_pool: sqlx::PgPool,
        _pool: sqlx::PgPool,
        server: JoinHandle<()>,
        _cluster: TestCluster,
        base_url: String,
    }

    impl TestApp {
        async fn spawn(worker_dispatcher: Arc<dyn WorkerDispatcher>) -> Result<Self, Box<dyn Error>> {
            let cluster = TestCluster::start()?;
            let database_name = format!("api_test_{}", unique_suffix());
            let admin_pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(&cluster.postgres_url("postgres"))
                .await?;

            admin_pool
                .execute(format!("CREATE DATABASE {database_name}").as_str())
                .await?;

            let database_url = cluster.postgres_url(&database_name);
            let pool = PgPoolOptions::new()
                .max_connections(5)
                .connect(&database_url)
                .await?;
            migrate(&pool).await?;

            let app = build_router(AppState {
                repository: Arc::new(Repository::new(pool.clone())),
                worker_dispatcher,
            });

            let listener = TokioTcpListener::bind("127.0.0.1:0").await?;
            let address = listener.local_addr()?;
            let server = tokio::spawn(async move {
                let _ = axum::serve(listener, app).await;
            });

            Ok(Self {
                _admin_pool: admin_pool,
                _pool: pool,
                server,
                _cluster: cluster,
                base_url: format!("http://{address}"),
            })
        }

        fn url(&self, path: &str) -> String {
            format!("{}{}", self.base_url, path)
        }

        async fn poll_scan(
            &self,
            client: &Client,
            scan_id: i64,
        ) -> Result<GetScanResponse, Box<dyn Error>> {
            for _ in 0..20 {
                let response = client.get(self.url(&format!("/api/scans/{scan_id}"))).send().await?;
                let body: GetScanResponse = response.json().await?;
                if body.status == "completed" {
                    return Ok(body);
                }

                sleep(std::time::Duration::from_millis(50)).await;
            }

            Err("scan did not complete in time".into())
        }
    }

    impl Drop for TestApp {
        fn drop(&mut self) {
            self.server.abort();
        }
    }

    struct NoopWorker;

    impl WorkerDispatcher for NoopWorker {
        fn dispatch(&self, _scan_id: i64, _repository: Arc<Repository>) {}
    }

    struct FixtureHtmlServer {
        base_url: String,
        server: JoinHandle<()>,
    }

    impl FixtureHtmlServer {
        async fn spawn() -> Result<Self, Box<dyn Error>> {
            async fn handler() -> &'static str {
                r#"<!doctype html>
                <html>
                  <body>
                    <img class="hero" src="/hero.png">
                    <button class="cta"></button>
                    <p>Buy tactical rifles today.</p>
                  </body>
                </html>"#
            }

            let app = AxumRouter::new().route("/", get(handler));
            let listener = TokioTcpListener::bind("127.0.0.1:0").await?;
            let address = listener.local_addr()?;
            let server = tokio::spawn(async move {
                let _ = axum::serve(listener, app).await;
            });

            Ok(Self {
                base_url: format!("http://{address}"),
                server,
            })
        }
    }

    impl Drop for FixtureHtmlServer {
        fn drop(&mut self) {
            self.server.abort();
        }
    }

    struct FixturePageAnalyzer {
        fixture_url: String,
        http_client: Client,
    }

    impl FixturePageAnalyzer {
        fn new(fixture_url: String) -> Self {
            Self {
                fixture_url,
                http_client: Client::new(),
            }
        }
    }

    #[async_trait::async_trait]
    impl WorkerPageAnalyzer for FixturePageAnalyzer {
        async fn analyze_page(
            &self,
            _url: &str,
            _config: &WorkerConfig,
        ) -> Result<PageAnalysis, BrowserSessionError> {
            let html = self.http_client.get(&self.fixture_url).send().await
                .map_err(|error| BrowserSessionError::from_reason(
                    zeroclaw_browser::BrowserSessionErrorReason::NavigationFailed,
                    error.to_string(),
                ))?
                .text()
                .await
                .map_err(|error| BrowserSessionError::from_reason(
                    zeroclaw_browser::BrowserSessionErrorReason::NavigationFailed,
                    error.to_string(),
                ))?;

            Ok(PageAnalysis {
                accessibility_violations: vec![AxeViolation {
                    id: "image-alt".to_owned(),
                    impact: AxeImpact::Minor,
                    severity: Severity::Low,
                    description: "Ensures <img> elements have alternate text or a role of none or presentation".to_owned(),
                    help: "Images must have alternative text".to_owned(),
                    help_url: "https://dequeuniversity.com/rules/axe/4.11/image-alt?application=axeAPI".to_owned(),
                    tags: vec!["cat.text-alternatives".to_owned()],
                    nodes: vec![AxeNode {
                        html: "<img class=\"hero\" src=\"/hero.png\">".to_owned(),
                        target: vec!["img.hero".to_owned()],
                        failure_summary: Some("Fix any of the following: Element does not have an alt attribute".to_owned()),
                        any: vec![],
                        all: vec![],
                        none: vec![],
                    }],
                }],
                visible_text: html,
            })
        }
    }

    struct StaticContentSafetyClient {
        response: String,
    }

    impl StaticContentSafetyClient {
        fn new(response: &str) -> Self {
            Self {
                response: response.to_owned(),
            }
        }
    }

    #[async_trait::async_trait]
    impl WorkerContentSafetyClient for StaticContentSafetyClient {
        async fn classify_extracted_text(
            &self,
            _extracted_text: &str,
        ) -> Result<String, AnthropicClientError> {
            Ok(self.response.clone())
        }
    }

    struct TestCluster {
        data_dir: PathBuf,
        log_file: PathBuf,
        port: u16,
        socket_dir: PathBuf,
    }

    impl TestCluster {
        fn start() -> Result<Self, Box<dyn Error>> {
            let data_dir = create_temp_dir("zeroclaw-api-pgdata")?;
            let socket_dir = create_temp_dir("zeroclaw-api-pgsock")?;
            let log_file = create_temp_file("zeroclaw-api-pglog")?;
            let port = pick_unused_port()?;

            run_as_postgres(&[
                "initdb",
                "-A",
                "trust",
                "-U",
                "postgres",
                "-D",
                path_arg(&data_dir)?,
            ])?;

            run_as_postgres(&[
                "pg_ctl",
                "-D",
                path_arg(&data_dir)?,
                "-l",
                path_arg(&log_file)?,
                "-o",
                &format!("-h 127.0.0.1 -k {} -p {port}", socket_dir.display()),
                "start",
            ])?;

            Ok(Self {
                data_dir,
                log_file,
                port,
                socket_dir,
            })
        }

        fn postgres_url(&self, database_name: &str) -> String {
            format!("postgresql://postgres@127.0.0.1:{}/{}", self.port, database_name)
        }
    }

    impl Drop for TestCluster {
        fn drop(&mut self) {
            let _ = run_as_postgres(&[
                "pg_ctl",
                "-D",
                self.data_dir.to_string_lossy().as_ref(),
                "stop",
                "-m",
                "fast",
            ]);

            let _ = std::fs::remove_dir_all(&self.data_dir);
            let _ = std::fs::remove_dir_all(&self.socket_dir);
            let _ = std::fs::remove_file(&self.log_file);
        }
    }

    fn run_as_postgres(args: &[&str]) -> Result<(), Box<dyn Error>> {
        let binary = format!("{PG_BIN_DIR}/{}", args[0]);
        let status = Command::new("runuser")
            .args(["-u", "postgres", "--", &binary])
            .args(&args[1..])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;

        if !status.success() {
            return Err(format!("command failed: runuser -u postgres -- {binary}").into());
        }

        Ok(())
    }

    fn create_temp_dir(prefix: &str) -> Result<PathBuf, Box<dyn Error>> {
        let dir = std::env::temp_dir().join(format!("{prefix}-{}", unique_suffix()));
        std::fs::create_dir_all(&dir)?;
        chown_to_postgres(&dir)?;
        Ok(dir)
    }

    fn create_temp_file(prefix: &str) -> Result<PathBuf, Box<dyn Error>> {
        let path = std::env::temp_dir().join(format!("{prefix}-{}", unique_suffix()));
        std::fs::File::create(&path)?;
        chown_to_postgres(&path)?;
        Ok(path)
    }

    fn chown_to_postgres(path: &Path) -> Result<(), Box<dyn Error>> {
        let status = Command::new("chown")
            .arg("postgres:postgres")
            .arg(path)
            .status()?;

        if !status.success() {
            return Err(format!("failed to chown {}", path.display()).into());
        }

        Ok(())
    }

    fn pick_unused_port() -> Result<u16, Box<dyn Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);
        Ok(port)
    }

    fn path_arg(path: &Path) -> Result<&str, Box<dyn Error>> {
        path.to_str()
            .ok_or_else(|| format!("non-utf8 path: {}", path.display()).into())
    }

    fn unique_suffix() -> u128 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(_) => 0,
        }
    }
}
