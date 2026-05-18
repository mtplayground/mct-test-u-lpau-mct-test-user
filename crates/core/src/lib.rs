mod enums;
mod models;
mod scoring;
mod validation;

pub use enums::{
    Category, FindingKind, InvalidEnumValue, RiskLevel, ScanPhase, ScanStatus, Severity,
};
pub use models::{Finding, NewFinding, NewScan, Scan, ScanScoreUpdate, ScanStatusUpdate};
pub use scoring::{
    aggregate_findings, category_breakdown, compute_inappropriate_score, compute_risk_level,
    recommendations_text, severity_weight, AggregatedFindings,
};
pub use validation::{
    validate_scan_url, validate_scan_url_with_resolver, UrlValidationError, ValidatedUrl,
};
