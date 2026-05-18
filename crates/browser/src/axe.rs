use serde::Deserialize;
use zeroclaw_core::Severity;

pub const AXE_SOURCE: &str = include_str!("../assets/axe.min.js");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxeViolation {
    pub id: String,
    pub impact: AxeImpact,
    pub severity: Severity,
    pub description: String,
    pub help: String,
    pub help_url: String,
    pub tags: Vec<String>,
    pub nodes: Vec<AxeNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxeNode {
    pub html: String,
    pub target: Vec<String>,
    pub failure_summary: Option<String>,
    pub any: Vec<AxeCheck>,
    pub all: Vec<AxeCheck>,
    pub none: Vec<AxeCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxeCheck {
    pub id: String,
    pub impact: Option<AxeImpact>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxeImpact {
    Minor,
    Moderate,
    Serious,
    Critical,
}

impl AxeImpact {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Minor => "minor",
            Self::Moderate => "moderate",
            Self::Serious => "serious",
            Self::Critical => "critical",
        }
    }

    pub fn severity(self) -> Severity {
        match self {
            Self::Minor => Severity::Low,
            Self::Moderate => Severity::Medium,
            Self::Serious => Severity::High,
            Self::Critical => Severity::Critical,
        }
    }
}

pub fn parse_axe_violations(json: &str) -> Result<Vec<AxeViolation>, AxeParseError> {
    let result: RawAxeRunResult =
        serde_json::from_str(json).map_err(AxeParseError::InvalidJson)?;
    parse_raw_axe_result(result)
}

pub(crate) fn parse_axe_result_value(
    value: serde_json::Value,
) -> Result<Vec<AxeViolation>, AxeParseError> {
    let result: RawAxeRunResult =
        serde_json::from_value(value).map_err(AxeParseError::InvalidJson)?;
    parse_raw_axe_result(result)
}

fn parse_raw_axe_result(result: RawAxeRunResult) -> Result<Vec<AxeViolation>, AxeParseError> {
    result
        .violations
        .into_iter()
        .map(parse_violation)
        .collect()
}

fn parse_violation(raw: RawAxeViolation) -> Result<AxeViolation, AxeParseError> {
    let impact = parse_impact(raw.impact.as_deref())?;

    Ok(AxeViolation {
        id: raw.id,
        impact,
        severity: impact.severity(),
        description: raw.description,
        help: raw.help,
        help_url: raw.help_url,
        tags: raw.tags,
        nodes: raw.nodes.into_iter().map(parse_node).collect::<Result<_, _>>()?,
    })
}

fn parse_node(raw: RawAxeNode) -> Result<AxeNode, AxeParseError> {
    Ok(AxeNode {
        html: raw.html,
        target: raw.target,
        failure_summary: raw.failure_summary,
        any: raw.any.into_iter().map(parse_check).collect::<Result<_, _>>()?,
        all: raw.all.into_iter().map(parse_check).collect::<Result<_, _>>()?,
        none: raw.none.into_iter().map(parse_check).collect::<Result<_, _>>()?,
    })
}

fn parse_check(raw: RawAxeCheck) -> Result<AxeCheck, AxeParseError> {
    Ok(AxeCheck {
        id: raw.id,
        impact: raw
            .impact
            .as_deref()
            .map(|value| parse_impact(Some(value)))
            .transpose()?,
        message: raw.message,
    })
}

fn parse_impact(impact: Option<&str>) -> Result<AxeImpact, AxeParseError> {
    match impact {
        Some("minor") => Ok(AxeImpact::Minor),
        Some("moderate") => Ok(AxeImpact::Moderate),
        Some("serious") => Ok(AxeImpact::Serious),
        Some("critical") => Ok(AxeImpact::Critical),
        Some(value) => Err(AxeParseError::UnsupportedImpact(value.to_owned())),
        None => Err(AxeParseError::MissingImpact),
    }
}

#[derive(Debug)]
pub enum AxeParseError {
    InvalidJson(serde_json::Error),
    MissingImpact,
    UnsupportedImpact(String),
}

impl std::fmt::Display for AxeParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(f, "invalid axe-core JSON: {error}"),
            Self::MissingImpact => write!(f, "axe-core violation is missing impact"),
            Self::UnsupportedImpact(impact) => {
                write!(f, "unsupported axe-core impact '{impact}'")
            }
        }
    }
}

impl std::error::Error for AxeParseError {}

#[derive(Debug, Deserialize)]
struct RawAxeRunResult {
    #[serde(default)]
    violations: Vec<RawAxeViolation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAxeViolation {
    id: String,
    impact: Option<String>,
    description: String,
    help: String,
    help_url: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    nodes: Vec<RawAxeNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAxeNode {
    html: String,
    #[serde(default)]
    target: Vec<String>,
    failure_summary: Option<String>,
    #[serde(default)]
    any: Vec<RawAxeCheck>,
    #[serde(default)]
    all: Vec<RawAxeCheck>,
    #[serde(default)]
    none: Vec<RawAxeCheck>,
}

#[derive(Debug, Deserialize)]
struct RawAxeCheck {
    id: String,
    impact: Option<String>,
    message: Option<String>,
}

#[cfg(test)]
mod tests {
    use zeroclaw_core::Severity;

    use super::{parse_axe_violations, AxeImpact, AxeParseError};

    const FIXTURE_JSON: &str = include_str!("../tests/fixtures/axe-violations.json");

    #[test]
    fn parser_reads_fixture_and_maps_impact_to_severity() {
        let violations = parse_axe_violations(FIXTURE_JSON)
            .expect("fixture JSON should parse into axe violations");

        assert_eq!(violations.len(), 4);
        assert_eq!(violations[0].id, "image-alt");
        assert_eq!(violations[0].impact, AxeImpact::Minor);
        assert_eq!(violations[0].severity, Severity::Low);
        assert_eq!(violations[1].severity, Severity::Medium);
        assert_eq!(violations[2].severity, Severity::High);
        assert_eq!(violations[3].severity, Severity::Critical);
        assert_eq!(
            violations[3].nodes[0].failure_summary.as_deref(),
            Some("Fix any of the following:\n  Ensures the contrast between foreground and background colors meets WCAG 2 AA minimum contrast ratio thresholds")
        );
        assert_eq!(
            violations[2].nodes[0].any[0].impact,
            Some(AxeImpact::Serious)
        );
    }

    #[test]
    fn parser_rejects_unknown_impact() {
        let result = parse_axe_violations(
            r#"{"violations":[{"id":"x","impact":"severe","description":"d","help":"h","helpUrl":"u","nodes":[]}]}"#,
        );

        assert!(matches!(
            result,
            Err(AxeParseError::UnsupportedImpact(value)) if value == "severe"
        ));
    }
}
