use std::collections::BTreeMap;

use crate::{Category, RiskLevel};

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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{Category, RiskLevel};

    use super::{category_breakdown, compute_risk_level};

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
}
