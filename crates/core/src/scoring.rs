use std::collections::BTreeMap;

use crate::{Category, NewFinding, RiskLevel, Severity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregatedFindings {
    pub accessibility_score: i32,
    pub inappropriate_score: i32,
    pub risk_level: RiskLevel,
    pub category_breakdown: BTreeMap<Category, usize>,
    pub recommendations_text: String,
}

pub fn compute_risk_level(total: usize) -> RiskLevel {
    match total {
        0..=2 => RiskLevel::Low,
        3..=6 => RiskLevel::Medium,
        7..=12 => RiskLevel::High,
        _ => RiskLevel::Critical,
    }
}

pub fn category_breakdown<I>(findings: I) -> BTreeMap<Category, usize>
where
    I: IntoIterator<Item = Category>,
{
    let mut breakdown: BTreeMap<Category, usize> = Category::ALL
        .into_iter()
        .map(|category| (category, 0))
        .collect();

    for category in findings {
        *breakdown.entry(category).or_insert(0) += 1;
    }

    breakdown
}

pub fn aggregate_findings(
    accessibility_findings: &[NewFinding],
    content_safety_findings: &[NewFinding],
) -> AggregatedFindings {
    let accessibility_score = accessibility_findings.len() as i32;
    let inappropriate_score = compute_inappropriate_score(
        content_safety_findings
            .iter()
            .map(|finding| finding.severity),
    );

    let mut categories = accessibility_findings
        .iter()
        .map(|_| Category::Accessibility)
        .collect::<Vec<_>>();
    categories.extend(
        content_safety_findings
            .iter()
            .map(|finding| finding.category),
    );

    let category_breakdown = category_breakdown(categories);
    let recommendations_text = recommendations_text(&category_breakdown);
    let risk_level = compute_risk_level(inappropriate_score.max(0) as usize);

    AggregatedFindings {
        accessibility_score,
        inappropriate_score,
        risk_level,
        category_breakdown,
        recommendations_text,
    }
}

pub fn compute_inappropriate_score<I>(severities: I) -> i32
where
    I: IntoIterator<Item = Severity>,
{
    severities.into_iter().map(severity_weight).sum()
}

pub fn severity_weight(severity: Severity) -> i32 {
    match severity {
        Severity::Low => 1,
        Severity::Medium => 3,
        Severity::High => 8,
        Severity::Critical => 13,
    }
}

pub fn recommendations_text(breakdown: &BTreeMap<Category, usize>) -> String {
    let mut recommendations = Vec::new();

    if breakdown
        .get(&Category::Accessibility)
        .copied()
        .unwrap_or_default()
        > 0
    {
        recommendations.push(
            "Resolve accessibility violations first, prioritizing repeated selector-level issues."
                .to_owned(),
        );
    }

    for (category, count) in breakdown.iter().filter(|(_, count)| **count > 0) {
        if *category == Category::Accessibility {
            continue;
        }

        recommendations.push(format!(
            "Review {} {} finding{} and reduce or remove the flagged content.",
            count,
            category.as_str().replace('_', " "),
            if *count == 1 { "" } else { "s" }
        ));
    }

    if recommendations.is_empty() {
        "No immediate remediation is required based on the current findings.".to_owned()
    } else {
        recommendations.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{Category, FindingKind, NewFinding, RiskLevel, Severity};

    use super::{
        aggregate_findings, category_breakdown, compute_inappropriate_score, compute_risk_level,
        recommendations_text, severity_weight,
    };

    #[test]
    fn compute_risk_level_uses_low_band_boundaries() {
        assert_eq!(compute_risk_level(0), RiskLevel::Low);
        assert_eq!(compute_risk_level(2), RiskLevel::Low);
    }

    #[test]
    fn compute_risk_level_uses_medium_band_boundaries() {
        assert_eq!(compute_risk_level(3), RiskLevel::Medium);
        assert_eq!(compute_risk_level(6), RiskLevel::Medium);
    }

    #[test]
    fn compute_risk_level_uses_high_band_boundaries() {
        assert_eq!(compute_risk_level(7), RiskLevel::High);
        assert_eq!(compute_risk_level(12), RiskLevel::High);
    }

    #[test]
    fn compute_risk_level_uses_critical_band_boundaries() {
        assert_eq!(compute_risk_level(13), RiskLevel::Critical);
        assert_eq!(compute_risk_level(99), RiskLevel::Critical);
    }

    #[test]
    fn category_breakdown_returns_empty_map_for_empty_findings() {
        let breakdown = category_breakdown(Vec::<Category>::new());

        let expected = Category::ALL
            .into_iter()
            .map(|category| (category, 0))
            .collect::<BTreeMap<_, _>>();

        assert_eq!(breakdown, expected);
    }

    #[test]
    fn category_breakdown_counts_each_category() {
        let breakdown = category_breakdown([
            Category::Accessibility,
            Category::Weapons,
            Category::Accessibility,
            Category::ScamsAndFraud,
            Category::Weapons,
            Category::Weapons,
        ]);

        let mut expected = Category::ALL
            .into_iter()
            .map(|category| (category, 0))
            .collect::<BTreeMap<_, _>>();

        expected.insert(Category::Accessibility, 2);
        expected.insert(Category::ScamsAndFraud, 1);
        expected.insert(Category::Weapons, 3);

        assert_eq!(breakdown, expected);
    }

    #[test]
    fn severity_weight_matches_risk_band_buckets() {
        assert_eq!(severity_weight(Severity::Low), 1);
        assert_eq!(severity_weight(Severity::Medium), 3);
        assert_eq!(severity_weight(Severity::High), 8);
        assert_eq!(severity_weight(Severity::Critical), 13);
    }

    #[test]
    fn inappropriate_score_sums_severity_weights() {
        let score = compute_inappropriate_score([
            Severity::Low,
            Severity::Medium,
            Severity::High,
            Severity::Critical,
        ]);

        assert_eq!(score, 25);
    }

    #[test]
    fn aggregate_findings_builds_scores_breakdown_and_recommendations() {
        let accessibility_findings = vec![new_finding(Category::Accessibility, Severity::Low)];
        let content_safety_findings = vec![
            new_finding(Category::Weapons, Severity::High),
            new_finding(Category::Profanity, Severity::Medium),
        ];

        let aggregated = aggregate_findings(&accessibility_findings, &content_safety_findings);

        assert_eq!(aggregated.accessibility_score, 1);
        assert_eq!(aggregated.inappropriate_score, 11);
        assert_eq!(aggregated.risk_level, RiskLevel::High);
        assert_eq!(aggregated.category_breakdown[&Category::Accessibility], 1);
        assert_eq!(aggregated.category_breakdown[&Category::Weapons], 1);
        assert!(aggregated
            .recommendations_text
            .contains("Resolve accessibility violations first"));
        assert!(aggregated
            .recommendations_text
            .contains("Review 1 weapons finding"));
    }

    #[test]
    fn recommendations_text_handles_empty_breakdown() {
        let breakdown = category_breakdown(Vec::<Category>::new());

        assert_eq!(
            recommendations_text(&breakdown),
            "No immediate remediation is required based on the current findings."
        );
    }

    fn new_finding(category: Category, severity: Severity) -> NewFinding {
        NewFinding {
            kind: if category == Category::Accessibility {
                FindingKind::Accessibility
            } else {
                FindingKind::ContentSafety
            },
            title: "title".to_owned(),
            category,
            severity,
            summary: "summary".to_owned(),
            location: None,
            suggestion: None,
            example_excerpt: None,
            why_unsafe: None,
        }
    }
}
