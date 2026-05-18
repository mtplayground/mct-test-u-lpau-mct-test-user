use std::borrow::Cow;

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use url::Url;

pub const DEFAULT_ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com/v1";
pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-sonnet-4-6";
pub const MAX_EXTRACTED_TEXT_BYTES: usize = 30 * 1024;
const ANTHROPIC_VERSION: &str = "2023-06-01";
const SYSTEM_PROMPT: &str = concat!(
    "You are a website safety classifier. ",
    "Review the provided website text and return strict JSON only. ",
    "Do not include markdown, code fences, commentary, or prose outside the JSON object. ",
    "Classify findings only into these 13 categories: ",
    "adult_sexual_content, child_sexual_exploitation, graphic_violence, self_harm, ",
    "hate_or_harassment, extremism, illegal_activities, weapons, drugs, gambling, ",
    "scams_and_fraud, misinformation, profanity. ",
    "Return a JSON object with this shape exactly: ",
    "{\"summary\":string,\"findings\":[{\"category\":string,\"severity\":\"low|medium|high|critical\",",
    "\"title\":string,\"summary\":string,\"example_excerpt\":string|null,\"why_unsafe\":string,",
    "\"recommended_action\":string}]}."
);

#[derive(Debug, Clone)]
pub struct AnthropicClientConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl AnthropicClientConfig {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: DEFAULT_ANTHROPIC_BASE_URL.to_owned(),
            model: DEFAULT_ANTHROPIC_MODEL.to_owned(),
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
}

#[derive(Debug, Clone)]
pub struct AnthropicClient {
    config: AnthropicClientConfig,
    http_client: Client,
}

impl AnthropicClient {
    pub fn new(config: AnthropicClientConfig) -> Self {
        Self {
            config,
            http_client: Client::new(),
        }
    }

    pub async fn classify_extracted_text(
        &self,
        extracted_text: &str,
    ) -> Result<String, AnthropicClientError> {
        let endpoint = messages_url(&self.config.base_url)?;
        let payload = build_messages_request(&self.config.model, extracted_text);
        let response = self
            .http_client
            .post(endpoint)
            .header("content-type", "application/json")
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&payload)
            .send()
            .await
            .map_err(AnthropicClientError::request)?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(AnthropicClientError::response_read)?;

        if !status.is_success() {
            return Err(AnthropicClientError::api_status(status, body));
        }

        extract_response_text(&body)
    }
}

fn messages_url(base_url: &str) -> Result<Url, AnthropicClientError> {
    let mut base = Url::parse(base_url).map_err(AnthropicClientError::invalid_base_url)?;
    if !base.path().ends_with('/') {
        let path = format!("{}/", base.path());
        base.set_path(&path);
    }

    base.join("messages")
        .map_err(AnthropicClientError::invalid_base_url)
}

fn build_messages_request(model: &str, extracted_text: &str) -> MessagesRequest {
    MessagesRequest {
        model: model.to_owned(),
        max_tokens: 2048,
        system: SYSTEM_PROMPT.to_owned(),
        messages: vec![Message {
            role: "user".to_owned(),
            content: build_user_message(extracted_text),
        }],
    }
}

fn build_user_message(extracted_text: &str) -> String {
    let text = truncate_extracted_text(extracted_text);
    format!(
        "Analyze this extracted website text and respond with strict JSON only.\n\nWebsite text:\n{}",
        text
    )
}

fn truncate_extracted_text(extracted_text: &str) -> Cow<'_, str> {
    if extracted_text.len() <= MAX_EXTRACTED_TEXT_BYTES {
        return Cow::Borrowed(extracted_text);
    }

    let mut cutoff = MAX_EXTRACTED_TEXT_BYTES;
    while !extracted_text.is_char_boundary(cutoff) {
        cutoff -= 1;
    }

    let mut truncated = extracted_text[..cutoff].to_owned();
    truncated.push_str("\n\n[truncated]");
    Cow::Owned(truncated)
}

fn extract_response_text(body: &str) -> Result<String, AnthropicClientError> {
    let response: MessagesResponse =
        serde_json::from_str(body).map_err(AnthropicClientError::invalid_response_json)?;

    let text = response
        .content
        .into_iter()
        .find_map(|block| match block {
            ContentBlock::Text { text } => Some(text),
            ContentBlock::Other => None,
        })
        .ok_or_else(AnthropicClientError::missing_text_block)?;

    Ok(text)
}

#[derive(Debug, Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u16,
    system: String,
    messages: Vec<Message>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock {
    Text { text: String },
    #[serde(other)]
    Other,
}

#[derive(Debug)]
pub struct AnthropicClientError {
    kind: AnthropicClientErrorKind,
    message: String,
}

impl AnthropicClientError {
    fn api_status(status: StatusCode, body: String) -> Self {
        Self::new(
            AnthropicClientErrorKind::ApiStatus,
            format!("Anthropic API returned {status}: {body}"),
        )
    }

    fn invalid_base_url(source: impl std::fmt::Display) -> Self {
        Self::new(
            AnthropicClientErrorKind::InvalidBaseUrl,
            format!("invalid Anthropic base URL: {source}"),
        )
    }

    fn invalid_response_json(source: serde_json::Error) -> Self {
        Self::new(
            AnthropicClientErrorKind::InvalidResponseJson,
            format!("invalid Anthropic response JSON: {source}"),
        )
    }

    fn missing_text_block() -> Self {
        Self::new(
            AnthropicClientErrorKind::MissingTextBlock,
            "Anthropic response did not include a text content block".to_owned(),
        )
    }

    fn request(source: reqwest::Error) -> Self {
        Self::new(
            AnthropicClientErrorKind::RequestFailed,
            format!("failed to call Anthropic API: {source}"),
        )
    }

    fn response_read(source: reqwest::Error) -> Self {
        Self::new(
            AnthropicClientErrorKind::ResponseReadFailed,
            format!("failed to read Anthropic response body: {source}"),
        )
    }

    fn new(kind: AnthropicClientErrorKind, message: String) -> Self {
        Self { kind, message }
    }

    pub fn kind(&self) -> AnthropicClientErrorKind {
        self.kind
    }
}

impl std::fmt::Display for AnthropicClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AnthropicClientError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnthropicClientErrorKind {
    ApiStatus,
    InvalidBaseUrl,
    InvalidResponseJson,
    MissingTextBlock,
    RequestFailed,
    ResponseReadFailed,
}

#[cfg(test)]
mod tests {
    use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};
    use serde_json::{json, Value};
    use tokio::net::TcpListener;

    use super::{
        build_messages_request, extract_response_text, truncate_extracted_text, AnthropicClient,
        AnthropicClientConfig, AnthropicClientErrorKind, DEFAULT_ANTHROPIC_MODEL,
        MAX_EXTRACTED_TEXT_BYTES, SYSTEM_PROMPT,
    };

    #[test]
    fn prompt_enumerates_categories_and_requires_strict_json() {
        assert!(SYSTEM_PROMPT.contains("adult_sexual_content"));
        assert!(SYSTEM_PROMPT.contains("child_sexual_exploitation"));
        assert!(SYSTEM_PROMPT.contains("graphic_violence"));
        assert!(SYSTEM_PROMPT.contains("self_harm"));
        assert!(SYSTEM_PROMPT.contains("hate_or_harassment"));
        assert!(SYSTEM_PROMPT.contains("extremism"));
        assert!(SYSTEM_PROMPT.contains("illegal_activities"));
        assert!(SYSTEM_PROMPT.contains("weapons"));
        assert!(SYSTEM_PROMPT.contains("drugs"));
        assert!(SYSTEM_PROMPT.contains("gambling"));
        assert!(SYSTEM_PROMPT.contains("scams_and_fraud"));
        assert!(SYSTEM_PROMPT.contains("misinformation"));
        assert!(SYSTEM_PROMPT.contains("profanity"));
        assert!(SYSTEM_PROMPT.contains("strict JSON only"));
    }

    #[test]
    fn truncates_extracted_text_to_limit_without_breaking_utf8() {
        let input = "é".repeat(MAX_EXTRACTED_TEXT_BYTES);
        let truncated = truncate_extracted_text(&input);

        assert!(truncated.len() > MAX_EXTRACTED_TEXT_BYTES);
        assert!(truncated.ends_with("\n\n[truncated]"));
        assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
    }

    #[test]
    fn request_uses_default_model_and_truncated_user_message() {
        let payload = build_messages_request(
            DEFAULT_ANTHROPIC_MODEL,
            &"a".repeat(MAX_EXTRACTED_TEXT_BYTES + 50),
        );

        assert_eq!(payload.model, DEFAULT_ANTHROPIC_MODEL);
        assert_eq!(payload.messages.len(), 1);
        assert!(payload.messages[0].content.contains("Website text:"));
        assert!(payload.messages[0].content.ends_with("\n\n[truncated]"));
    }

    #[test]
    fn extracts_first_text_block() {
        let raw = r#"{
            "content": [
                {"type":"text","text":"{\"summary\":\"ok\",\"findings\":[]}"},
                {"type":"text","text":"ignored"}
            ]
        }"#;

        let result = extract_response_text(raw).expect("text block should be extracted");

        assert_eq!(result, "{\"summary\":\"ok\",\"findings\":[]}");
    }

    #[tokio::test]
    async fn client_calls_messages_endpoint_and_returns_raw_json() {
        #[derive(Clone)]
        struct TestState;

        async fn handler(
            State(_state): State<TestState>,
            headers: HeaderMap,
            Json(payload): Json<Value>,
        ) -> Json<Value> {
            assert_eq!(
                headers.get("x-api-key").and_then(|value| value.to_str().ok()),
                Some("test-key")
            );
            assert_eq!(
                headers
                    .get("anthropic-version")
                    .and_then(|value| value.to_str().ok()),
                Some("2023-06-01")
            );
            assert_eq!(payload["model"], "custom-model");
            assert_eq!(
                payload["messages"][0]["content"]
                    .as_str()
                    .expect("user content must be a string"),
                "Analyze this extracted website text and respond with strict JSON only.\n\nWebsite text:\nhello world"
            );

            Json(json!({
                "content": [
                    {
                        "type": "text",
                        "text": "{\"summary\":\"safe\",\"findings\":[]}"
                    }
                ]
            }))
        }

        let app = Router::new()
            .route("/v1/messages", post(handler))
            .with_state(TestState);
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test listener should bind");
        let address = listener
            .local_addr()
            .expect("test listener should expose an address");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let client = AnthropicClient::new(
            AnthropicClientConfig::new("test-key")
                .with_base_url(format!("http://{address}/v1"))
                .with_model("custom-model"),
        );

        let result = client
            .classify_extracted_text("hello world")
            .await
            .expect("mock Anthropic request should succeed");

        assert_eq!(result, "{\"summary\":\"safe\",\"findings\":[]}");
    }

    #[test]
    fn reports_missing_text_block() {
        let error = extract_response_text(r#"{"content":[{"type":"tool_use","id":"x"}]}"#)
            .expect_err("missing text block should fail");

        assert_eq!(error.kind(), AnthropicClientErrorKind::MissingTextBlock);
    }
}
