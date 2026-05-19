use time::OffsetDateTime;

use crate::{Category, FindingKind, RiskLevel, ScanPhase, ScanStatus, Severity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scan {
    pub id: i64,
    pub url: String,
    pub normalized_url: String,
    pub status: ScanStatus,
    pub phase: ScanPhase,
    pub accessibility_score: Option<i32>,
    pub inappropriate_score: Option<i32>,
    pub risk_level: Option<RiskLevel>,
    pub content_safety_skipped: bool,
    pub error_reason: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewScan {
    pub url: String,
    pub normalized_url: String,
    pub status: ScanStatus,
    pub phase: ScanPhase,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanStatusUpdate {
    pub status: ScanStatus,
    pub phase: ScanPhase,
    pub error_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanScoreUpdate {
    pub accessibility_score: Option<i32>,
    pub inappropriate_score: Option<i32>,
    pub risk_level: Option<RiskLevel>,
    pub content_safety_skipped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub id: i64,
    pub scan_id: i64,
    pub kind: FindingKind,
    pub title: String,
    pub category: Category,
    pub severity: Severity,
    pub summary: String,
    pub location: Option<String>,
    pub suggestion: Option<String>,
    pub example_excerpt: Option<String>,
    pub why_unsafe: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFinding {
    pub kind: FindingKind,
    pub title: String,
    pub category: Category,
    pub severity: Severity,
    pub summary: String,
    pub location: Option<String>,
    pub suggestion: Option<String>,
    pub example_excerpt: Option<String>,
    pub why_unsafe: Option<String>,
}
