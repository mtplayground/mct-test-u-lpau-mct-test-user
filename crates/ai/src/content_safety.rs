use serde::Deserialize;
use zeroclaw_core::{Category, FindingKind, NewFinding, Severity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentSafetyFinding {
    pub title: String,
    pub category: ContentSafetyCategory,
    pub severity: Severity,
    pub summary: String,
    pub example_excerpt: Option<String>,
    pub why_unsafe: String,
    pub suggested_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedContentSafetyResponse {
    pub summary: String,
    pub findings: Vec<ContentSafetyFinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentSafetyCategory {
    AdultSexualContent,
    ChildSexualExploitation,
    GraphicViolence,
    SelfHarm,
    HateOrHarassment,
    Extremism,
    IllegalActivities,
    Weapons,
    Drugs,
    Gambling,
    ScamsAndFraud,
    Misinformation,
    Profanity,
}

impl ContentSafetyCategory {
    fn into_domain(self) -> Category {
        match self {
            Self::AdultSexualContent => Category::AdultSexualContent,
            Self::ChildSexualExploitation => Category::ChildSexualExploitation,
            Self::GraphicViolence => Category::GraphicViolence,
            Self::SelfHarm => Category::SelfHarm,
            Self::HateOrHarassment => Category::HateOrHarassment,
            Self::Extremism => Category::Extremism,
            Self::IllegalActivities => Category::IllegalActivities,
            Self::Weapons => Category::Weapons,
            Self::Drugs => Category::Drugs,
            Self::Gambling => Category::Gambling,
            Self::ScamsAndFraud => Category::ScamsAndFraud,
            Self::Misinformation => Category::Misinformation,
            Self::Profanity => Category::Profanity,
        }
    }
}

pub fn parse_content_safety_response(
    response: &str,
) -> Result<ParsedContentSafetyResponse, ContentSafetyParseError> {
    let json_block =
        extract_json_object(response).ok_or(ContentSafetyParseError::MissingJsonObject)?;
    let parsed: RawContentSafetyResponse =
        serde_json::from_str(json_block).map_err(ContentSafetyParseError::InvalidJson)?;

    Ok(ParsedContentSafetyResponse {
        summary: parsed.summary,
        findings: parsed.findings.into_iter().map(Into::into).collect(),
    })
}

pub fn map_content_safety_findings(findings: &[ContentSafetyFinding]) -> Vec<NewFinding> {
    findings
        .iter()
        .map(|finding| NewFinding {
            kind: FindingKind::ContentSafety,
            title: finding.title.clone(),
            category: finding.category.into_domain(),
            severity: finding.severity,
            summary: finding.summary.clone(),
            location: None,
            suggestion: Some(finding.suggested_action.clone()),
            example_excerpt: finding.example_excerpt.clone(),
            why_unsafe: Some(finding.why_unsafe.clone()),
        })
        .collect()
}

fn extract_json_object(input: &str) -> Option<&str> {
    let mut start = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, ch) in input.char_indices() {
        if start.is_none() {
            if ch == '{' {
                start = Some(index);
                depth = 1;
            }
            continue;
        }

        if in_string {
            if escaped {
                escaped = false;
                continue;
            }

            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }

            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let start_index = start.expect("start must exist when depth reaches zero");
                    return Some(&input[start_index..=index]);
                }
            }
            _ => {}
        }
    }

    None
}

#[derive(Debug)]
pub enum ContentSafetyParseError {
    InvalidJson(serde_json::Error),
    MissingJsonObject,
}

impl std::fmt::Display for ContentSafetyParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(f, "invalid content-safety JSON: {error}"),
            Self::MissingJsonObject => write!(f, "response did not contain a JSON object"),
        }
    }
}

impl std::error::Error for ContentSafetyParseError {}

#[derive(Debug, Deserialize)]
struct RawContentSafetyResponse {
    summary: String,
    #[serde(default)]
    findings: Vec<RawContentSafetyFinding>,
}

#[derive(Debug, Deserialize)]
struct RawContentSafetyFinding {
    title: String,
    category: ContentSafetyCategory,
    severity: RawSeverity,
    summary: String,
    example_excerpt: Option<String>,
    why_unsafe: String,
    #[serde(alias = "recommended_action")]
    suggested_action: String,
}

impl<'de> Deserialize<'de> for ContentSafetyCategory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "adult_sexual_content" => Ok(Self::AdultSexualContent),
            "child_sexual_exploitation" => Ok(Self::ChildSexualExploitation),
            "graphic_violence" => Ok(Self::GraphicViolence),
            "self_harm" => Ok(Self::SelfHarm),
            "hate_or_harassment" => Ok(Self::HateOrHarassment),
            "extremism" => Ok(Self::Extremism),
            "illegal_activities" => Ok(Self::IllegalActivities),
            "weapons" => Ok(Self::Weapons),
            "drugs" => Ok(Self::Drugs),
            "gambling" => Ok(Self::Gambling),
            "scams_and_fraud" => Ok(Self::ScamsAndFraud),
            "misinformation" => Ok(Self::Misinformation),
            "profanity" => Ok(Self::Profanity),
            other => Err(serde::de::Error::custom(format!(
                "unsupported content-safety category '{other}'"
            ))),
        }
    }
}

impl From<RawContentSafetyFinding> for ContentSafetyFinding {
    fn from(value: RawContentSafetyFinding) -> Self {
        Self {
            title: value.title,
            category: value.category,
            severity: value.severity.into(),
            summary: value.summary,
            example_excerpt: value.example_excerpt,
            why_unsafe: value.why_unsafe,
            suggested_action: value.suggested_action,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl From<RawSeverity> for Severity {
    fn from(value: RawSeverity) -> Self {
        match value {
            RawSeverity::Low => Severity::Low,
            RawSeverity::Medium => Severity::Medium,
            RawSeverity::High => Severity::High,
            RawSeverity::Critical => Severity::Critical,
        }
    }
}

#[cfg(test)]
mod tests {
    use zeroclaw_core::{Category, FindingKind, Severity};

    use super::{
        map_content_safety_findings, parse_content_safety_response, ContentSafetyParseError,
    };

    const CLEAN_FIXTURE: &str = include_str!("../tests/fixtures/content-safety-clean.json");
    const MALFORMED_FIXTURE: &str = include_str!("../tests/fixtures/content-safety-malformed.txt");
    const EMPTY_FIXTURE: &str = include_str!("../tests/fixtures/content-safety-empty.txt");

    #[test]
    fn parses_clean_response_with_leading_and_trailing_prose() {
        let parsed = parse_content_safety_response(CLEAN_FIXTURE)
            .expect("clean fixture should parse successfully");

        assert_eq!(parsed.summary, "The page contains weapon sales language.");
        assert_eq!(parsed.findings.len(), 2);
        assert_eq!(parsed.findings[0].title, "Weapon sales promotion");
        assert_eq!(parsed.findings[0].severity, Severity::High);
        assert_eq!(parsed.findings[1].severity, Severity::Medium);
    }

    #[test]
    fn maps_findings_into_domain_records() {
        let parsed = parse_content_safety_response(CLEAN_FIXTURE)
            .expect("clean fixture should parse successfully");

        let mapped = map_content_safety_findings(&parsed.findings);

        assert_eq!(mapped.len(), 2);
        assert_eq!(mapped[0].kind, FindingKind::ContentSafety);
        assert_eq!(mapped[0].category, Category::Weapons);
        assert_eq!(mapped[0].severity, Severity::High);
        assert_eq!(mapped[0].location, None);
        assert_eq!(
            mapped[0].suggestion.as_deref(),
            Some("Remove direct sales language and add clear policy restrictions.")
        );
        assert_eq!(
            mapped[0].example_excerpt.as_deref(),
            Some("Buy tactical rifles today with fast shipping.")
        );
        assert_eq!(
            mapped[0].why_unsafe.as_deref(),
            Some("It promotes acquiring real-world weapons.")
        );
    }

    #[test]
    fn rejects_malformed_response() {
        let error = parse_content_safety_response(MALFORMED_FIXTURE)
            .expect_err("malformed fixture should fail to parse");

        assert!(matches!(error, ContentSafetyParseError::InvalidJson(_)));
    }

    #[test]
    fn parses_empty_findings_response() {
        let parsed = parse_content_safety_response(EMPTY_FIXTURE)
            .expect("empty fixture should parse successfully");

        assert_eq!(parsed.summary, "No policy issues detected.");
        assert!(parsed.findings.is_empty());
        assert!(map_content_safety_findings(&parsed.findings).is_empty());
    }
}
