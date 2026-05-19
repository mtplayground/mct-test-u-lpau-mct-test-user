use std::{path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use url::Url;
use zeroclaw_ai::{
    map_content_safety_findings, parse_content_safety_response, AnthropicClient,
    AnthropicClientError, AnthropicClientErrorKind,
};
use zeroclaw_browser::{
    map_accessibility_findings, AxeViolation, BrowserSession, BrowserSessionConfig,
    BrowserSessionError, BrowserSessionErrorReason,
};
use zeroclaw_core::{
    aggregate_findings, compute_risk_level, validate_scan_url, NewFinding, RiskLevel, Scan,
    ScanPhase, ScanScoreUpdate, ScanStatus, ScanStatusUpdate, UrlValidationError,
};
use zeroclaw_storage::{Repository, RepositoryError};

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub chromium_path: PathBuf,
    pub scan_timeout: Duration,
    pub allow_private_urls: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageAnalysis {
    pub accessibility_violations: Vec<AxeViolation>,
    pub visible_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRunOutput {
    pub accessibility_findings: Vec<NewFinding>,
    pub accessibility_score: i32,
    pub content_safety_findings: Vec<NewFinding>,
    pub inappropriate_score: i32,
    pub risk_level: RiskLevel,
    pub category_breakdown: std::collections::BTreeMap<zeroclaw_core::Category, usize>,
    pub recommendations_text: String,
    pub content_safety_summary: String,
    pub content_safety_skipped: bool,
}

pub struct ScanWorker<S, P, C> {
    store: Arc<S>,
    page_analyzer: Arc<P>,
    content_safety_client: Option<Arc<C>>,
    config: WorkerConfig,
}

impl<S, P, C> ScanWorker<S, P, C> {
    pub fn new(
        store: Arc<S>,
        page_analyzer: Arc<P>,
        content_safety_client: Option<Arc<C>>,
        config: WorkerConfig,
    ) -> Self {
        Self {
            store,
            page_analyzer,
            content_safety_client,
            config,
        }
    }
}

impl<S, P, C> ScanWorker<S, P, C>
where
    S: ScanStore,
    P: PageAnalyzer,
    C: ContentSafetyClient,
{
    pub async fn run_scan(&self, scan_id: i64) -> Result<ScanRunOutput, ScanWorkerError> {
        let scan = self
            .store
            .find_scan_by_id(scan_id)
            .await
            .map_err(ScanWorkerError::store)?
            .ok_or(ScanWorkerError::scan_not_found(scan_id))?;

        let validated_url = match normalize_scan_url(&scan.url, self.config.allow_private_urls) {
            Ok(validated) => validated,
            Err(error) => {
                let failure = ScanFailureReason::from_url_validation_error(&error);
                self.mark_failed(scan_id, failure).await?;
                return Err(ScanWorkerError::pipeline(failure, error.to_string()));
            }
        };

        self.update_phase(scan_id, ScanPhase::Loading).await?;

        let page_analysis = match self
            .page_analyzer
            .analyze_page(&validated_url, &self.config)
            .await
        {
            Ok(result) => result,
            Err(error) => {
                let failure = ScanFailureReason::from_browser_error(&error);
                self.mark_failed(scan_id, failure).await?;
                return Err(ScanWorkerError::pipeline(failure, error.to_string()));
            }
        };

        self.update_phase(scan_id, ScanPhase::Accessibility).await?;
        let accessibility_findings = map_accessibility_findings(&page_analysis.accessibility_violations);

        if page_analysis.visible_text.trim().is_empty() {
            let failure = ScanFailureReason::NoContent;
            self.mark_failed(scan_id, failure).await?;
            return Err(ScanWorkerError::pipeline(
                failure,
                "page did not produce visible text content".to_owned(),
            ));
        }

        let Some(content_safety_client) = self.content_safety_client.as_ref() else {
            let content_safety_findings = Vec::new();
            let mut aggregated =
                aggregate_findings(&accessibility_findings, &content_safety_findings);
            aggregated.risk_level = compute_risk_level(aggregated.accessibility_score as usize);

            self.update_phase(scan_id, ScanPhase::Aggregating).await?;
            self.store
                .insert_findings_batch(scan_id, &accessibility_findings)
                .await
                .map_err(ScanWorkerError::store)?;
            self.store
                .update_scan_scores(
                    scan_id,
                    &ScanScoreUpdate {
                        accessibility_score: Some(aggregated.accessibility_score),
                        inappropriate_score: Some(0),
                        risk_level: Some(aggregated.risk_level),
                        content_safety_skipped: true,
                    },
                )
                .await
                .map_err(ScanWorkerError::store)?
                .ok_or(ScanWorkerError::scan_not_found(scan_id))?;
            self.store
                .update_scan_status(
                    scan_id,
                    &ScanStatusUpdate {
                        status: ScanStatus::Completed,
                        phase: ScanPhase::Completed,
                        error_reason: None,
                    },
                )
                .await
                .map_err(ScanWorkerError::store)?
                .ok_or(ScanWorkerError::scan_not_found(scan_id))?;

            return Ok(ScanRunOutput {
                accessibility_findings,
                accessibility_score: aggregated.accessibility_score,
                content_safety_findings,
                inappropriate_score: 0,
                risk_level: aggregated.risk_level,
                category_breakdown: aggregated.category_breakdown,
                recommendations_text: aggregated.recommendations_text,
                content_safety_summary: String::new(),
                content_safety_skipped: true,
            });
        };

        self.update_phase(scan_id, ScanPhase::ContentSafety).await?;
        let raw_response = match content_safety_client
            .classify_extracted_text(&page_analysis.visible_text)
            .await
        {
            Ok(raw_response) => raw_response,
            Err(error) => {
                let failure = ScanFailureReason::from_anthropic_error(&error);
                self.mark_failed(scan_id, failure).await?;
                return Err(ScanWorkerError::pipeline(failure, error.to_string()));
            }
        };

        let parsed = match parse_content_safety_response(&raw_response) {
            Ok(parsed) => parsed,
            Err(error) => {
                let failure = ScanFailureReason::Blocked;
                self.mark_failed(scan_id, failure).await?;
                return Err(ScanWorkerError::pipeline(failure, error.to_string()));
            }
        };

        let content_safety_findings = map_content_safety_findings(&parsed.findings);
        let aggregated = aggregate_findings(&accessibility_findings, &content_safety_findings);

        self.update_phase(scan_id, ScanPhase::Aggregating).await?;
        self.store
            .insert_findings_batch(scan_id, &accessibility_findings)
            .await
            .map_err(ScanWorkerError::store)?;
        self.store
            .insert_findings_batch(scan_id, &content_safety_findings)
            .await
            .map_err(ScanWorkerError::store)?;
        self.store
            .update_scan_scores(
                scan_id,
                &ScanScoreUpdate {
                    accessibility_score: Some(aggregated.accessibility_score),
                    inappropriate_score: Some(aggregated.inappropriate_score),
                    risk_level: Some(aggregated.risk_level),
                    content_safety_skipped: false,
                },
            )
            .await
            .map_err(ScanWorkerError::store)?
            .ok_or(ScanWorkerError::scan_not_found(scan_id))?;
        self.store
            .update_scan_status(
                scan_id,
                &ScanStatusUpdate {
                    status: ScanStatus::Completed,
                    phase: ScanPhase::Completed,
                    error_reason: None,
                },
            )
            .await
            .map_err(ScanWorkerError::store)?
            .ok_or(ScanWorkerError::scan_not_found(scan_id))?;

        Ok(ScanRunOutput {
            accessibility_findings,
            accessibility_score: aggregated.accessibility_score,
            content_safety_findings,
            inappropriate_score: aggregated.inappropriate_score,
            risk_level: aggregated.risk_level,
            category_breakdown: aggregated.category_breakdown,
            recommendations_text: aggregated.recommendations_text,
            content_safety_summary: parsed.summary,
            content_safety_skipped: false,
        })
    }

    async fn mark_failed(
        &self,
        scan_id: i64,
        reason: ScanFailureReason,
    ) -> Result<(), ScanWorkerError> {
        self.store
            .update_scan_status(
                scan_id,
                &ScanStatusUpdate {
                    status: ScanStatus::Failed,
                    phase: ScanPhase::Failed,
                    error_reason: Some(reason.as_error_reason().to_owned()),
                },
            )
            .await
            .map_err(ScanWorkerError::store)?
            .ok_or(ScanWorkerError::scan_not_found(scan_id))?;

        Ok(())
    }

    async fn update_phase(&self, scan_id: i64, phase: ScanPhase) -> Result<(), ScanWorkerError> {
        self.store
            .update_scan_status(
                scan_id,
                &ScanStatusUpdate {
                    status: ScanStatus::Running,
                    phase,
                    error_reason: None,
                },
            )
            .await
            .map_err(ScanWorkerError::store)?
            .ok_or(ScanWorkerError::scan_not_found(scan_id))?;

        Ok(())
    }
}

fn normalize_scan_url(input: &str, allow_private_urls: bool) -> Result<String, UrlValidationError> {
    if !allow_private_urls {
        return validate_scan_url(input).map(|validated| validated.normalized_url);
    }

    let url = Url::parse(input).map_err(UrlValidationError::InvalidUrl)?;

    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(UrlValidationError::UnsupportedScheme {
                scheme: scheme.to_owned(),
            });
        }
    }

    match url.host_str() {
        Some(host) if !host.trim().is_empty() => Ok(url.to_string()),
        _ => Err(UrlValidationError::EmptyHost),
    }
}

#[async_trait]
pub trait ScanStore: Send + Sync {
    async fn find_scan_by_id(&self, scan_id: i64) -> Result<Option<Scan>, RepositoryError>;
    async fn insert_findings_batch(
        &self,
        scan_id: i64,
        findings: &[NewFinding],
    ) -> Result<Vec<zeroclaw_core::Finding>, RepositoryError>;
    async fn update_scan_scores(
        &self,
        scan_id: i64,
        update: &ScanScoreUpdate,
    ) -> Result<Option<Scan>, RepositoryError>;

    async fn update_scan_status(
        &self,
        scan_id: i64,
        update: &ScanStatusUpdate,
    ) -> Result<Option<Scan>, RepositoryError>;
}

#[async_trait]
impl ScanStore for Repository {
    async fn find_scan_by_id(&self, scan_id: i64) -> Result<Option<Scan>, RepositoryError> {
        Repository::find_scan_by_id(self, scan_id).await
    }

    async fn insert_findings_batch(
        &self,
        scan_id: i64,
        findings: &[NewFinding],
    ) -> Result<Vec<zeroclaw_core::Finding>, RepositoryError> {
        Repository::insert_findings_batch(self, scan_id, findings).await
    }

    async fn update_scan_scores(
        &self,
        scan_id: i64,
        update: &ScanScoreUpdate,
    ) -> Result<Option<Scan>, RepositoryError> {
        Repository::update_scan_scores(self, scan_id, update).await
    }

    async fn update_scan_status(
        &self,
        scan_id: i64,
        update: &ScanStatusUpdate,
    ) -> Result<Option<Scan>, RepositoryError> {
        Repository::update_scan_status(self, scan_id, update).await
    }
}

#[async_trait]
pub trait PageAnalyzer: Send + Sync {
    async fn analyze_page(
        &self,
        url: &str,
        config: &WorkerConfig,
    ) -> Result<PageAnalysis, BrowserSessionError>;
}

#[derive(Debug, Default, Clone)]
pub struct ChromiumPageAnalyzer;

#[async_trait]
impl PageAnalyzer for ChromiumPageAnalyzer {
    async fn analyze_page(
        &self,
        url: &str,
        config: &WorkerConfig,
    ) -> Result<PageAnalysis, BrowserSessionError> {
        let session = BrowserSession::launch(BrowserSessionConfig::new(
            config.chromium_path.clone(),
            config.scan_timeout,
        ))
        .await?;

        session.navigate(url).await?;
        let accessibility_violations = session.run_axe().await?;
        let visible_text = session.extract_visible_text().await?;
        session.close().await?;

        Ok(PageAnalysis {
            accessibility_violations,
            visible_text,
        })
    }
}

#[async_trait]
pub trait ContentSafetyClient: Send + Sync {
    async fn classify_extracted_text(
        &self,
        extracted_text: &str,
    ) -> Result<String, AnthropicClientError>;
}

#[async_trait]
impl ContentSafetyClient for AnthropicClient {
    async fn classify_extracted_text(
        &self,
        extracted_text: &str,
    ) -> Result<String, AnthropicClientError> {
        AnthropicClient::classify_extracted_text(self, extracted_text).await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanFailureReason {
    InvalidUrl,
    Unreachable,
    Blocked,
    Timeout,
    NoContent,
}

impl ScanFailureReason {
    pub fn as_error_reason(self) -> &'static str {
        match self {
            Self::InvalidUrl => "invalid_url",
            Self::Unreachable => "unreachable",
            Self::Blocked => "blocked",
            Self::Timeout => "timeout",
            Self::NoContent => "no_content",
        }
    }

    fn from_url_validation_error(_error: &UrlValidationError) -> Self {
        Self::InvalidUrl
    }

    fn from_browser_error(error: &BrowserSessionError) -> Self {
        match error.reason() {
            BrowserSessionErrorReason::NavigationTimeout => Self::Timeout,
            BrowserSessionErrorReason::ResponseTooLarge
            | BrowserSessionErrorReason::AxeInjectionFailed
            | BrowserSessionErrorReason::AxeResultParseFailed
            | BrowserSessionErrorReason::AxeScanFailed
            | BrowserSessionErrorReason::ScriptEvaluationFailed
            | BrowserSessionErrorReason::TextExtractionFailed => Self::Blocked,
            BrowserSessionErrorReason::BrowserLaunchFailed
            | BrowserSessionErrorReason::BrowserCloseFailed
            | BrowserSessionErrorReason::HttpClientBuildFailed
            | BrowserSessionErrorReason::InvalidBrowserConfig
            | BrowserSessionErrorReason::NavigationFailed => Self::Unreachable,
        }
    }

    fn from_anthropic_error(error: &AnthropicClientError) -> Self {
        match error.kind() {
            AnthropicClientErrorKind::RequestFailed | AnthropicClientErrorKind::ResponseReadFailed => {
                Self::Unreachable
            }
            AnthropicClientErrorKind::ApiStatus
            | AnthropicClientErrorKind::InvalidResponseJson
            | AnthropicClientErrorKind::MissingTextBlock
            | AnthropicClientErrorKind::InvalidBaseUrl => Self::Blocked,
        }
    }
}

#[derive(Debug)]
pub struct ScanWorkerError {
    kind: ScanWorkerErrorKind,
    message: String,
}

impl ScanWorkerError {
    fn pipeline(reason: ScanFailureReason, message: String) -> Self {
        Self {
            kind: ScanWorkerErrorKind::PipelineFailed(reason),
            message,
        }
    }

    fn scan_not_found(scan_id: i64) -> Self {
        Self {
            kind: ScanWorkerErrorKind::ScanNotFound,
            message: format!("scan {scan_id} was not found"),
        }
    }

    fn store(source: RepositoryError) -> Self {
        Self {
            kind: ScanWorkerErrorKind::Store,
            message: format!("worker storage operation failed: {source}"),
        }
    }

    pub fn kind(&self) -> ScanWorkerErrorKind {
        self.kind
    }
}

impl std::fmt::Display for ScanWorkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ScanWorkerError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanWorkerErrorKind {
    PipelineFailed(ScanFailureReason),
    ScanNotFound,
    Store,
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use zeroclaw_ai::AnthropicClientError;
    use zeroclaw_browser::{AxeImpact, AxeNode, AxeViolation};
    use zeroclaw_core::{Finding, Severity};

    use super::*;

    #[tokio::test]
    async fn run_scan_updates_phases_and_completes() {
        let store = Arc::new(FakeStore::with_scan(sample_scan("https://example.com")));
        let worker = ScanWorker::new(
            store.clone(),
            Arc::new(FakePageAnalyzer::success(PageAnalysis {
                accessibility_violations: vec![sample_violation()],
                visible_text: "Visible content".to_owned(),
            })),
            Some(Arc::new(FakeContentSafetyClient::success(
                r#"{"summary":"Safe enough","findings":[]}"#.to_owned(),
            ))),
            sample_config(),
        );

        let output = worker.run_scan(7).await.expect("worker should succeed");

        assert_eq!(output.accessibility_score, 1);
        assert_eq!(output.accessibility_findings.len(), 1);
        assert!(output.content_safety_findings.is_empty());
        assert_eq!(output.inappropriate_score, 0);
        assert_eq!(output.risk_level, RiskLevel::Low);
        assert_eq!(output.category_breakdown[&zeroclaw_core::Category::Accessibility], 1);
        assert!(output
            .recommendations_text
            .contains("Resolve accessibility violations first"));
        assert_eq!(output.content_safety_summary, "Safe enough");
        assert!(!output.content_safety_skipped);
        assert_eq!(store.inserted_findings_len(), 1);
        assert_eq!(
            store.score_updates(),
            vec![(Some(1), Some(0), Some(RiskLevel::Low), false)]
        );

        let updates = store.status_updates();
        assert_eq!(
            updates,
            vec![
                (ScanStatus::Running, ScanPhase::Loading, None),
                (ScanStatus::Running, ScanPhase::Accessibility, None),
                (ScanStatus::Running, ScanPhase::ContentSafety, None),
                (ScanStatus::Running, ScanPhase::Aggregating, None),
                (ScanStatus::Completed, ScanPhase::Completed, None),
            ]
        );
    }

    #[tokio::test]
    async fn run_scan_skips_content_safety_when_client_is_absent() {
        let store = Arc::new(FakeStore::with_scan(sample_scan("https://example.com")));
        let worker = ScanWorker::<FakeStore, FakePageAnalyzer, FakeContentSafetyClient>::new(
            store.clone(),
            Arc::new(FakePageAnalyzer::success(PageAnalysis {
                accessibility_violations: vec![
                    sample_violation(),
                    sample_violation(),
                    sample_violation(),
                ],
                visible_text: "Visible content".to_owned(),
            })),
            None,
            sample_config(),
        );

        let output = worker.run_scan(7).await.expect("worker should succeed");

        assert_eq!(output.accessibility_score, 3);
        assert_eq!(output.accessibility_findings.len(), 3);
        assert!(output.content_safety_findings.is_empty());
        assert_eq!(output.inappropriate_score, 0);
        assert_eq!(output.risk_level, RiskLevel::Medium);
        assert!(output.content_safety_skipped);
        assert_eq!(output.content_safety_summary, "");
        assert_eq!(store.inserted_findings_len(), 3);
        assert_eq!(
            store.score_updates(),
            vec![(Some(3), Some(0), Some(RiskLevel::Medium), true)]
        );

        let updates = store.status_updates();
        assert_eq!(
            updates,
            vec![
                (ScanStatus::Running, ScanPhase::Loading, None),
                (ScanStatus::Running, ScanPhase::Accessibility, None),
                (ScanStatus::Running, ScanPhase::Aggregating, None),
                (ScanStatus::Completed, ScanPhase::Completed, None),
            ]
        );
        assert!(
            updates
                .iter()
                .all(|(_, phase, _)| *phase != ScanPhase::ContentSafety),
            "content safety phase should be skipped when no client is configured"
        );
    }

    #[tokio::test]
    async fn run_scan_marks_invalid_url_as_failed() {
        let store = Arc::new(FakeStore::with_scan(sample_scan("not-a-url")));
        let worker = ScanWorker::new(
            store.clone(),
            Arc::new(FakePageAnalyzer::unreachable()),
            Some(Arc::new(FakeContentSafetyClient::success(
                r#"{"summary":"unused","findings":[]}"#.to_owned(),
            ))),
            sample_config(),
        );

        let error = worker.run_scan(7).await.expect_err("worker should fail");

        assert_eq!(
            error.kind(),
            ScanWorkerErrorKind::PipelineFailed(ScanFailureReason::InvalidUrl)
        );
        assert_eq!(
            store.status_updates(),
            vec![(
                ScanStatus::Failed,
                ScanPhase::Failed,
                Some("invalid_url".to_owned())
            )]
        );
    }

    #[tokio::test]
    async fn run_scan_marks_timeout_failure() {
        let store = Arc::new(FakeStore::with_scan(sample_scan("https://example.com")));
        let worker = ScanWorker::new(
            store.clone(),
            Arc::new(FakePageAnalyzer::timeout()),
            Some(Arc::new(FakeContentSafetyClient::success(
                r#"{"summary":"unused","findings":[]}"#.to_owned(),
            ))),
            sample_config(),
        );

        let error = worker.run_scan(7).await.expect_err("worker should fail");

        assert_eq!(
            error.kind(),
            ScanWorkerErrorKind::PipelineFailed(ScanFailureReason::Timeout)
        );
        assert_eq!(
            store.status_updates(),
            vec![
                (ScanStatus::Running, ScanPhase::Loading, None),
                (
                    ScanStatus::Failed,
                    ScanPhase::Failed,
                    Some("timeout".to_owned())
                )
            ]
        );
    }

    #[tokio::test]
    async fn run_scan_marks_no_content_failure() {
        let store = Arc::new(FakeStore::with_scan(sample_scan("https://example.com")));
        let worker = ScanWorker::new(
            store.clone(),
            Arc::new(FakePageAnalyzer::success(PageAnalysis {
                accessibility_violations: vec![],
                visible_text: " \n\t ".to_owned(),
            })),
            Some(Arc::new(FakeContentSafetyClient::success(
                r#"{"summary":"unused","findings":[]}"#.to_owned(),
            ))),
            sample_config(),
        );

        let error = worker.run_scan(7).await.expect_err("worker should fail");

        assert_eq!(
            error.kind(),
            ScanWorkerErrorKind::PipelineFailed(ScanFailureReason::NoContent)
        );
        assert_eq!(
            store.status_updates(),
            vec![
                (ScanStatus::Running, ScanPhase::Loading, None),
                (ScanStatus::Running, ScanPhase::Accessibility, None),
                (
                    ScanStatus::Failed,
                    ScanPhase::Failed,
                    Some("no_content".to_owned())
                )
            ]
        );
    }

    #[test]
    fn maps_failure_reasons_to_expected_error_reason_values() {
        assert_eq!(ScanFailureReason::InvalidUrl.as_error_reason(), "invalid_url");
        assert_eq!(ScanFailureReason::Unreachable.as_error_reason(), "unreachable");
        assert_eq!(ScanFailureReason::Blocked.as_error_reason(), "blocked");
        assert_eq!(ScanFailureReason::Timeout.as_error_reason(), "timeout");
        assert_eq!(ScanFailureReason::NoContent.as_error_reason(), "no_content");
    }

    fn sample_config() -> WorkerConfig {
        WorkerConfig {
            chromium_path: PathBuf::from("/usr/bin/chromium"),
            scan_timeout: Duration::from_secs(30),
            allow_private_urls: false,
        }
    }

    fn sample_scan(url: &str) -> Scan {
        Scan {
            id: 7,
            url: url.to_owned(),
            normalized_url: url.to_owned(),
            status: ScanStatus::Pending,
            phase: ScanPhase::Queued,
            accessibility_score: None,
            inappropriate_score: None,
            risk_level: None,
            content_safety_skipped: false,
            error_reason: None,
            created_at: time::OffsetDateTime::UNIX_EPOCH,
            updated_at: time::OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn sample_violation() -> AxeViolation {
        AxeViolation {
            id: "image-alt".to_owned(),
            impact: AxeImpact::Moderate,
            severity: Severity::Medium,
            description: "Images need alt text".to_owned(),
            help: "Images must have alternative text".to_owned(),
            help_url: "https://example.com/help".to_owned(),
            tags: vec![],
            nodes: vec![AxeNode {
                html: "<img src=\"hero.png\">".to_owned(),
                target: vec!["img.hero".to_owned()],
                failure_summary: Some("Add alt text".to_owned()),
                any: vec![],
                all: vec![],
                none: vec![],
            }],
        }
    }

    struct FakeStore {
        scan: Mutex<Option<Scan>>,
        updates: Mutex<Vec<(ScanStatus, ScanPhase, Option<String>)>>,
        inserted_findings: Mutex<Vec<NewFinding>>,
        score_updates: Mutex<Vec<(Option<i32>, Option<i32>, Option<RiskLevel>, bool)>>,
    }

    impl FakeStore {
        fn with_scan(scan: Scan) -> Self {
            Self {
                scan: Mutex::new(Some(scan)),
                updates: Mutex::new(Vec::new()),
                inserted_findings: Mutex::new(Vec::new()),
                score_updates: Mutex::new(Vec::new()),
            }
        }

        fn status_updates(&self) -> Vec<(ScanStatus, ScanPhase, Option<String>)> {
            self.updates.lock().expect("updates lock poisoned").clone()
        }

        fn inserted_findings_len(&self) -> usize {
            self.inserted_findings
                .lock()
                .expect("findings lock poisoned")
                .len()
        }

        fn score_updates(&self) -> Vec<(Option<i32>, Option<i32>, Option<RiskLevel>, bool)> {
            self.score_updates
                .lock()
                .expect("score updates lock poisoned")
                .clone()
        }
    }

    #[async_trait]
    impl ScanStore for FakeStore {
        async fn find_scan_by_id(&self, _scan_id: i64) -> Result<Option<Scan>, RepositoryError> {
            Ok(self.scan.lock().expect("scan lock poisoned").clone())
        }

        async fn insert_findings_batch(
            &self,
            _scan_id: i64,
            findings: &[NewFinding],
        ) -> Result<Vec<Finding>, RepositoryError> {
            self.inserted_findings
                .lock()
                .expect("findings lock poisoned")
                .extend(findings.iter().cloned());

            Ok(findings
                .iter()
                .enumerate()
                .map(|(index, finding)| Finding {
                    id: index as i64 + 1,
                    scan_id: 7,
                    kind: finding.kind,
                    title: finding.title.clone(),
                    category: finding.category,
                    severity: finding.severity,
                    summary: finding.summary.clone(),
                    location: finding.location.clone(),
                    suggestion: finding.suggestion.clone(),
                    example_excerpt: finding.example_excerpt.clone(),
                    why_unsafe: finding.why_unsafe.clone(),
                })
                .collect())
        }

        async fn update_scan_scores(
            &self,
            _scan_id: i64,
            update: &ScanScoreUpdate,
        ) -> Result<Option<Scan>, RepositoryError> {
            self.score_updates
                .lock()
                .expect("score updates lock poisoned")
                .push((
                    update.accessibility_score,
                    update.inappropriate_score,
                    update.risk_level,
                    update.content_safety_skipped,
                ));

            let mut guard = self.scan.lock().expect("scan lock poisoned");
            if let Some(scan) = guard.as_mut() {
                scan.accessibility_score = update.accessibility_score;
                scan.inappropriate_score = update.inappropriate_score;
                scan.risk_level = update.risk_level;
                scan.content_safety_skipped = update.content_safety_skipped;
            }

            Ok(guard.clone())
        }

        async fn update_scan_status(
            &self,
            _scan_id: i64,
            update: &ScanStatusUpdate,
        ) -> Result<Option<Scan>, RepositoryError> {
            self.updates
                .lock()
                .expect("updates lock poisoned")
                .push((
                    update.status,
                    update.phase,
                    update.error_reason.clone(),
                ));

            let mut guard = self.scan.lock().expect("scan lock poisoned");
            if let Some(scan) = guard.as_mut() {
                scan.status = update.status;
                scan.phase = update.phase;
                scan.error_reason = update.error_reason.clone();
            }

            Ok(guard.clone())
        }
    }

    struct FakePageAnalyzer {
        result: Mutex<VecDeque<Result<PageAnalysis, BrowserSessionError>>>,
    }

    impl FakePageAnalyzer {
        fn success(result: PageAnalysis) -> Self {
            Self {
                result: Mutex::new(VecDeque::from([Ok(result)])),
            }
        }

        fn timeout() -> Self {
            Self {
                result: Mutex::new(VecDeque::from([Err(BrowserSessionError::from_reason(
                    BrowserSessionErrorReason::NavigationTimeout,
                    "timed out".to_owned(),
                ))])),
            }
        }

        fn unreachable() -> Self {
            Self {
                result: Mutex::new(VecDeque::from([Err(BrowserSessionError::from_reason(
                    BrowserSessionErrorReason::NavigationFailed,
                    "unreachable".to_owned(),
                ))])),
            }
        }
    }

    #[async_trait]
    impl PageAnalyzer for FakePageAnalyzer {
        async fn analyze_page(
            &self,
            _url: &str,
            _config: &WorkerConfig,
        ) -> Result<PageAnalysis, BrowserSessionError> {
            self.result
                .lock()
                .expect("page analyzer lock poisoned")
                .pop_front()
                .expect("page analyzer should have a queued result")
        }
    }

    struct FakeContentSafetyClient {
        result: Mutex<VecDeque<Result<String, AnthropicClientError>>>,
    }

    impl FakeContentSafetyClient {
        fn success(result: String) -> Self {
            Self {
                result: Mutex::new(VecDeque::from([Ok(result)])),
            }
        }
    }

    #[async_trait]
    impl ContentSafetyClient for FakeContentSafetyClient {
        async fn classify_extracted_text(
            &self,
            _extracted_text: &str,
        ) -> Result<String, AnthropicClientError> {
            self.result
                .lock()
                .expect("content safety lock poisoned")
                .pop_front()
                .expect("content safety client should have a queued result")
        }
    }
}
