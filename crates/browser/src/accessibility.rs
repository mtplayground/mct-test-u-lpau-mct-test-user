use zeroclaw_core::{Category, FindingKind, NewFinding};

use crate::axe::{AxeCheck, AxeNode, AxeViolation};

pub fn map_accessibility_findings(violations: &[AxeViolation]) -> Vec<NewFinding> {
    let mut findings = Vec::new();

    for violation in violations {
        for node in &violation.nodes {
            findings.push(NewFinding {
                kind: FindingKind::Accessibility,
                title: violation.help.clone(),
                category: Category::Accessibility,
                severity: violation.severity,
                summary: violation.description.clone(),
                location: Some(format_location(node)),
                suggestion: Some(format_suggestion(&violation.help_url, remediation_hint(node))),
                example_excerpt: Some(node.html.clone()),
                why_unsafe: None,
            });
        }
    }

    findings
}

pub fn accessibility_score(violations: &[AxeViolation]) -> i32 {
    violations
        .iter()
        .map(|violation| violation.nodes.len() as i32)
        .sum()
}

fn format_location(node: &AxeNode) -> String {
    let selector = if node.target.is_empty() {
        "unknown selector".to_owned()
    } else {
        node.target.join(", ")
    };

    format!("Selector: {selector}\nNode: {}", node.html)
}

fn format_suggestion(help_url: &str, remediation_hint: Option<String>) -> String {
    match remediation_hint {
        Some(hint) => format!("{help_url}\n\nRemediation hint: {hint}"),
        None => help_url.to_owned(),
    }
}

fn remediation_hint(node: &AxeNode) -> Option<String> {
    if let Some(summary) = node
        .failure_summary
        .as_deref()
        .and_then(clean_text)
    {
        return Some(summary);
    }

    let checks = node
        .any
        .iter()
        .chain(node.all.iter())
        .chain(node.none.iter())
        .filter_map(check_message)
        .collect::<Vec<_>>();

    if checks.is_empty() {
        None
    } else {
        Some(checks.join(" "))
    }
}

fn check_message(check: &AxeCheck) -> Option<String> {
    check.message.as_deref().and_then(clean_text)
}

fn clean_text(value: &str) -> Option<String> {
    let normalized = value
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

#[cfg(test)]
mod tests {
    use zeroclaw_core::{Category, FindingKind, Severity};

    use crate::axe::{parse_axe_violations, AxeCheck, AxeImpact, AxeNode, AxeViolation};

    use super::{accessibility_score, map_accessibility_findings};

    const FIXTURE_JSON: &str = include_str!("../tests/fixtures/axe-violations.json");

    #[test]
    fn maps_one_finding_per_node_from_fixture() {
        let violations = parse_axe_violations(FIXTURE_JSON)
            .expect("fixture JSON should parse into axe violations");

        let findings = map_accessibility_findings(&violations);

        assert_eq!(findings.len(), 4);
        assert_eq!(accessibility_score(&violations), 4);
        assert_eq!(findings[0].kind, FindingKind::Accessibility);
        assert_eq!(findings[0].category, Category::Accessibility);
        assert_eq!(findings[0].severity, Severity::Low);
        assert_eq!(findings[0].title, "Images must have alternative text");
        assert_eq!(
            findings[0].summary,
            "Ensures <img> elements have alternate text or a role of none or presentation"
        );
        assert_eq!(
            findings[0].location.as_deref(),
            Some("Selector: img.hero\nNode: <img class=\"hero\" src=\"/hero.png\">")
        );
        assert_eq!(
            findings[0].suggestion.as_deref(),
            Some("https://dequeuniversity.com/rules/axe/4.11/image-alt?application=axeAPI\n\nRemediation hint: Fix any of the following: Element does not have an alt attribute")
        );
        assert_eq!(
            findings[0].example_excerpt.as_deref(),
            Some("<img class=\"hero\" src=\"/hero.png\">")
        );
        assert_eq!(findings[0].why_unsafe, None);
    }

    #[test]
    fn maps_multiple_nodes_to_multiple_findings() {
        let violations = vec![AxeViolation {
            id: "label".to_owned(),
            impact: AxeImpact::Moderate,
            severity: Severity::Medium,
            description: "Ensures every form element has a label".to_owned(),
            help: "Form elements must have labels".to_owned(),
            help_url: "https://example.com/axe/label".to_owned(),
            tags: vec!["cat.forms".to_owned()],
            nodes: vec![
                AxeNode {
                    html: "<input id=\"email\">".to_owned(),
                    target: vec!["#email".to_owned()],
                    failure_summary: Some("Add a label".to_owned()),
                    any: vec![],
                    all: vec![],
                    none: vec![],
                },
                AxeNode {
                    html: "<input id=\"name\">".to_owned(),
                    target: vec!["#name".to_owned()],
                    failure_summary: None,
                    any: vec![AxeCheck {
                        id: "implicit-label".to_owned(),
                        impact: Some(AxeImpact::Moderate),
                        message: Some("Wrap the input in a label".to_owned()),
                    }],
                    all: vec![],
                    none: vec![],
                },
            ],
        }];

        let findings = map_accessibility_findings(&violations);

        assert_eq!(findings.len(), 2);
        assert_eq!(accessibility_score(&violations), 2);
        assert_eq!(
            findings[1].suggestion.as_deref(),
            Some("https://example.com/axe/label\n\nRemediation hint: Wrap the input in a label")
        );
    }
}
