mod enums;
mod models;
mod scoring;
mod validation;

pub use enums::{
    Category, FindingKind, InvalidEnumValue, RiskLevel, ScanPhase, ScanStatus, Severity,
};
pub use models::{Finding, NewFinding, NewScan, Scan, ScanScoreUpdate, ScanStatusUpdate};
pub use scoring::{category_breakdown, compute_risk_level};
pub use validation::{
    validate_scan_url, validate_scan_url_with_resolver, UrlValidationError, ValidatedUrl,
};
