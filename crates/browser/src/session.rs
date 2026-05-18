use std::{path::PathBuf, time::Duration};

use chromiumoxide::{Browser, BrowserConfig, Page};
use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use tokio::{task::JoinHandle, time::timeout};

use crate::axe::{parse_axe_result_value, AXE_SOURCE};

pub const MAX_RESPONSE_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct BrowserSessionConfig {
    pub chromium_path: PathBuf,
    pub scan_timeout: Duration,
}

impl BrowserSessionConfig {
    pub fn new(chromium_path: PathBuf, scan_timeout: Duration) -> Self {
        Self {
            chromium_path,
            scan_timeout,
        }
    }
}

#[derive(Debug)]
pub struct BrowserSession {
    browser: Browser,
    handler_task: JoinHandle<()>,
    navigation_timeout: Duration,
    page: Page,
    size_guard_client: reqwest::Client,
}

impl BrowserSession {
    pub async fn launch(config: BrowserSessionConfig) -> Result<Self, BrowserSessionError> {
        let browser_config = BrowserConfig::builder()
            .chrome_executable(&config.chromium_path)
            .disable_https_first()
            .no_sandbox()
            .request_timeout(config.scan_timeout)
            .launch_timeout(config.scan_timeout)
            .build()
            .map_err(BrowserSessionError::invalid_browser_config)?;

        let (browser, mut handler) = Browser::launch(browser_config)
            .await
            .map_err(BrowserSessionError::launch)?;

        let handler_task = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if let Err(error) = event {
                    tracing::warn!(error = %error, "chromium handler exited with an error");
                    break;
                }
            }
        });

        let page = browser
            .new_page("about:blank")
            .await
            .map_err(BrowserSessionError::launch)?;

        let size_guard_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .timeout(config.scan_timeout)
            .build()
            .map_err(BrowserSessionError::http_client)?;

        Ok(Self {
            browser,
            handler_task,
            navigation_timeout: config.scan_timeout,
            page,
            size_guard_client,
        })
    }

    pub async fn navigate(&self, url: &str) -> Result<(), BrowserSessionError> {
        enforce_response_size_limit(&self.size_guard_client, url, self.navigation_timeout).await?;

        timeout(self.navigation_timeout, self.page.goto(url))
            .await
            .map_err(|_| BrowserSessionError::navigation_timeout(self.navigation_timeout))?
            .map_err(BrowserSessionError::navigation)?;

        Ok(())
    }

    pub async fn evaluate_script<T>(&self, script: &str) -> Result<T, BrowserSessionError>
    where
        T: DeserializeOwned,
    {
        let evaluation = self
            .page
            .evaluate(script)
            .await
            .map_err(BrowserSessionError::script_evaluation)?;

        evaluation
            .into_value::<T>()
            .map_err(BrowserSessionError::script_deserialization)
    }

    pub async fn extract_visible_text(&self) -> Result<String, BrowserSessionError> {
        self.evaluate_script::<String>(
            r#"(() => {
                if (!document.body) {
                    return "";
                }

                return (document.body.innerText || "")
                    .split("\n")
                    .map((line) => line.trim())
                    .filter((line) => line.length > 0)
                    .join("\n");
            })()"#,
        )
        .await
        .map_err(|error| error.with_reason(BrowserSessionErrorReason::TextExtractionFailed))
    }

    pub async fn inject_axe(&self) -> Result<(), BrowserSessionError> {
        let script = format!(
            r#"(() => {{
                {AXE_SOURCE}
                return !!(window.axe && typeof window.axe.run === "function");
            }})()"#
        );

        let loaded = self
            .evaluate_script::<bool>(&script)
            .await
            .map_err(|error| error.with_reason(BrowserSessionErrorReason::AxeInjectionFailed))?;

        if loaded {
            Ok(())
        } else {
            Err(BrowserSessionError::axe_injection_failed(
                "axe injection completed but window.axe.run was unavailable".to_owned(),
            ))
        }
    }

    pub async fn run_axe(&self) -> Result<Vec<crate::axe::AxeViolation>, BrowserSessionError> {
        self.inject_axe().await?;

        let raw_result = self
            .evaluate_script::<serde_json::Value>(
                r#"(() => {
                    if (!window.axe || typeof window.axe.run !== "function") {
                        throw new Error("axe-core is not loaded");
                    }

                    return window.axe.run(document);
                })()"#,
            )
            .await
            .map_err(|error| error.with_reason(BrowserSessionErrorReason::AxeScanFailed))?;

        parse_axe_result_value(raw_result)
            .map_err(|error| BrowserSessionError::axe_result_parse_failed(error.to_string()))
    }

    pub async fn close(mut self) -> Result<(), BrowserSessionError> {
        self.browser
            .close()
            .await
            .map_err(BrowserSessionError::browser_close)?;
        self.handler_task.abort();
        Ok(())
    }
}

impl Drop for BrowserSession {
    fn drop(&mut self) {
        self.handler_task.abort();
    }
}

async fn enforce_response_size_limit(
    client: &reqwest::Client,
    url: &str,
    timeout_duration: Duration,
) -> Result<(), BrowserSessionError> {
    let response = timeout(timeout_duration, client.get(url).send())
        .await
        .map_err(|_| BrowserSessionError::navigation_timeout(timeout_duration))?
        .map_err(BrowserSessionError::navigation)?;

    if let Some(content_length) = response.content_length() {
        ensure_within_size_limit(content_length)?;
    }

    let mut bytes_read = 0_u64;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(BrowserSessionError::navigation)?;
        bytes_read = bytes_read.saturating_add(chunk.len() as u64);
        ensure_within_size_limit(bytes_read)?;
    }

    Ok(())
}

fn ensure_within_size_limit(size: u64) -> Result<(), BrowserSessionError> {
    if size > MAX_RESPONSE_BYTES {
        return Err(BrowserSessionError::response_too_large(size));
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserSessionErrorReason {
    AxeInjectionFailed,
    AxeResultParseFailed,
    AxeScanFailed,
    BrowserLaunchFailed,
    BrowserCloseFailed,
    HttpClientBuildFailed,
    InvalidBrowserConfig,
    NavigationFailed,
    NavigationTimeout,
    ResponseTooLarge,
    ScriptEvaluationFailed,
    TextExtractionFailed,
}

impl BrowserSessionErrorReason {
    pub fn as_error_reason(self) -> &'static str {
        match self {
            Self::AxeInjectionFailed => "axe_injection_failed",
            Self::AxeResultParseFailed => "axe_result_parse_failed",
            Self::AxeScanFailed => "axe_scan_failed",
            Self::BrowserLaunchFailed => "browser_launch_failed",
            Self::BrowserCloseFailed => "browser_close_failed",
            Self::HttpClientBuildFailed => "http_client_build_failed",
            Self::InvalidBrowserConfig => "invalid_browser_config",
            Self::NavigationFailed => "navigation_failed",
            Self::NavigationTimeout => "navigation_timeout",
            Self::ResponseTooLarge => "response_too_large",
            Self::ScriptEvaluationFailed => "script_evaluation_failed",
            Self::TextExtractionFailed => "text_extraction_failed",
        }
    }
}

#[derive(Debug)]
pub struct BrowserSessionError {
    reason: BrowserSessionErrorReason,
    message: String,
}

impl BrowserSessionError {
    fn axe_injection_failed(message: String) -> Self {
        Self::new(BrowserSessionErrorReason::AxeInjectionFailed, message)
    }

    fn axe_result_parse_failed(message: String) -> Self {
        Self::new(
            BrowserSessionErrorReason::AxeResultParseFailed,
            format!("failed to parse axe-core result: {message}"),
        )
    }

    fn browser_close(source: chromiumoxide::error::CdpError) -> Self {
        Self::new(
            BrowserSessionErrorReason::BrowserCloseFailed,
            format!("failed to close Chromium session: {source}"),
        )
    }

    fn http_client(source: reqwest::Error) -> Self {
        Self::new(
            BrowserSessionErrorReason::HttpClientBuildFailed,
            format!("failed to build HTTP client for size guard: {source}"),
        )
    }

    fn invalid_browser_config(source: String) -> Self {
        Self::new(
            BrowserSessionErrorReason::InvalidBrowserConfig,
            format!("invalid Chromium browser configuration: {source}"),
        )
    }

    fn launch(source: chromiumoxide::error::CdpError) -> Self {
        Self::new(
            BrowserSessionErrorReason::BrowserLaunchFailed,
            format!("failed to launch Chromium: {source}"),
        )
    }

    fn navigation(source: impl std::fmt::Display) -> Self {
        Self::new(
            BrowserSessionErrorReason::NavigationFailed,
            format!("failed to navigate page: {source}"),
        )
    }

    fn navigation_timeout(timeout: Duration) -> Self {
        Self::new(
            BrowserSessionErrorReason::NavigationTimeout,
            format!("page navigation exceeded timeout of {} seconds", timeout.as_secs()),
        )
    }

    fn response_too_large(size: u64) -> Self {
        Self::new(
            BrowserSessionErrorReason::ResponseTooLarge,
            format!(
                "page response exceeded the {} byte limit with {} bytes",
                MAX_RESPONSE_BYTES, size
            ),
        )
    }

    fn script_deserialization(source: serde_json::Error) -> Self {
        Self::new(
            BrowserSessionErrorReason::ScriptEvaluationFailed,
            format!("browser script returned an unexpected value: {source}"),
        )
    }

    fn script_evaluation(source: chromiumoxide::error::CdpError) -> Self {
        Self::new(
            BrowserSessionErrorReason::ScriptEvaluationFailed,
            format!("failed to evaluate browser script: {source}"),
        )
    }

    fn new(reason: BrowserSessionErrorReason, message: String) -> Self {
        Self { reason, message }
    }

    pub fn from_reason(reason: BrowserSessionErrorReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }

    pub fn error_reason(&self) -> &'static str {
        self.reason.as_error_reason()
    }

    pub fn reason(&self) -> BrowserSessionErrorReason {
        self.reason
    }

    fn with_reason(mut self, reason: BrowserSessionErrorReason) -> Self {
        self.reason = reason;
        self
    }
}

impl std::fmt::Display for BrowserSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for BrowserSessionError {}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        ensure_within_size_limit, BrowserSessionError, BrowserSessionErrorReason,
        MAX_RESPONSE_BYTES,
    };

    #[test]
    fn accepts_sizes_up_to_limit() {
        assert!(ensure_within_size_limit(MAX_RESPONSE_BYTES).is_ok());
    }

    #[test]
    fn rejects_sizes_above_limit() {
        let error = ensure_within_size_limit(MAX_RESPONSE_BYTES + 1)
            .expect_err("sizes above the limit should fail");

        assert_eq!(error.reason(), BrowserSessionErrorReason::ResponseTooLarge);
        assert_eq!(error.error_reason(), "response_too_large");
    }

    #[test]
    fn exposes_expected_error_reason_strings() {
        let timeout = BrowserSessionError::navigation_timeout(Duration::from_secs(45));
        let launch = BrowserSessionError::new(
            BrowserSessionErrorReason::BrowserLaunchFailed,
            "launch failed".to_owned(),
        );
        let extract = BrowserSessionError::new(
            BrowserSessionErrorReason::TextExtractionFailed,
            "extract failed".to_owned(),
        );

        assert_eq!(timeout.error_reason(), "navigation_timeout");
        assert_eq!(launch.error_reason(), "browser_launch_failed");
        assert_eq!(extract.error_reason(), "text_extraction_failed");
        assert_eq!(
            BrowserSessionErrorReason::AxeInjectionFailed.as_error_reason(),
            "axe_injection_failed"
        );
    }
}
