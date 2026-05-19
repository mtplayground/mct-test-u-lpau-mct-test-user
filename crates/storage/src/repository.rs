use std::str::FromStr;

use sqlx::{postgres::PgRow, PgPool, Row};
use zeroclaw_core::{
    Finding, InvalidEnumValue, NewFinding, NewScan, RiskLevel, Scan, ScanScoreUpdate,
    ScanStatus, ScanStatusUpdate,
};

#[derive(Debug, Clone)]
pub struct Repository {
    pool: PgPool,
}

impl Repository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert_scan(&self, new_scan: &NewScan) -> Result<Scan, RepositoryError> {
        let row = sqlx::query(
            r#"
            INSERT INTO scans (url, normalized_url, status, phase)
            VALUES ($1, $2, $3, $4)
            RETURNING
                id,
                url,
                normalized_url,
                status,
                phase,
                accessibility_score,
                inappropriate_score,
                risk_level,
                content_safety_skipped,
                error_reason,
                created_at,
                updated_at
            "#,
        )
        .bind(&new_scan.url)
        .bind(&new_scan.normalized_url)
        .bind(new_scan.status.as_str())
        .bind(new_scan.phase.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(RepositoryError::Sqlx)?;

        map_scan_row(row)
    }

    pub async fn update_scan_status(
        &self,
        scan_id: i64,
        update: &ScanStatusUpdate,
    ) -> Result<Option<Scan>, RepositoryError> {
        let row = sqlx::query(
            r#"
            UPDATE scans
            SET
                status = $2,
                phase = $3,
                error_reason = $4,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id,
                url,
                normalized_url,
                status,
                phase,
                accessibility_score,
                inappropriate_score,
                risk_level,
                content_safety_skipped,
                error_reason,
                created_at,
                updated_at
            "#,
        )
        .bind(scan_id)
        .bind(update.status.as_str())
        .bind(update.phase.as_str())
        .bind(&update.error_reason)
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::Sqlx)?;

        row.map(map_scan_row).transpose()
    }

    pub async fn update_scan_scores(
        &self,
        scan_id: i64,
        update: &ScanScoreUpdate,
    ) -> Result<Option<Scan>, RepositoryError> {
        let risk_level = update.risk_level.map(RiskLevel::as_str);
        let row = sqlx::query(
            r#"
            UPDATE scans
            SET
                accessibility_score = $2,
                inappropriate_score = $3,
                risk_level = $4,
                content_safety_skipped = $5,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id,
                url,
                normalized_url,
                status,
                phase,
                accessibility_score,
                inappropriate_score,
                risk_level,
                content_safety_skipped,
                error_reason,
                created_at,
                updated_at
            "#,
        )
        .bind(scan_id)
        .bind(update.accessibility_score)
        .bind(update.inappropriate_score)
        .bind(risk_level)
        .bind(update.content_safety_skipped)
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::Sqlx)?;

        row.map(map_scan_row).transpose()
    }

    pub async fn find_scan_by_id(&self, scan_id: i64) -> Result<Option<Scan>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                url,
                normalized_url,
                status,
                phase,
                accessibility_score,
                inappropriate_score,
                risk_level,
                content_safety_skipped,
                error_reason,
                created_at,
                updated_at
            FROM scans
            WHERE id = $1
            "#,
        )
        .bind(scan_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::Sqlx)?;

        row.map(map_scan_row).transpose()
    }

    pub async fn find_recent_completed_by_url(
        &self,
        normalized_url: &str,
    ) -> Result<Option<Scan>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                url,
                normalized_url,
                status,
                phase,
                accessibility_score,
                inappropriate_score,
                risk_level,
                content_safety_skipped,
                error_reason,
                created_at,
                updated_at
            FROM scans
            WHERE normalized_url = $1 AND status = $2
            ORDER BY updated_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(normalized_url)
        .bind(ScanStatus::Completed.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::Sqlx)?;

        row.map(map_scan_row).transpose()
    }

    pub async fn insert_findings_batch(
        &self,
        scan_id: i64,
        findings: &[NewFinding],
    ) -> Result<Vec<Finding>, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(RepositoryError::Sqlx)?;
        let mut inserted = Vec::with_capacity(findings.len());

        for finding in findings {
            let row = sqlx::query(
                r#"
                INSERT INTO findings (
                    scan_id,
                    kind,
                    title,
                    category,
                    severity,
                    summary,
                    location,
                    suggestion,
                    example_excerpt,
                    why_unsafe
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                RETURNING
                    id,
                    scan_id,
                    kind,
                    title,
                    category,
                    severity,
                    summary,
                    location,
                    suggestion,
                    example_excerpt,
                    why_unsafe
                "#,
            )
            .bind(scan_id)
            .bind(finding.kind.as_str())
            .bind(&finding.title)
            .bind(finding.category.as_str())
            .bind(finding.severity.as_str())
            .bind(&finding.summary)
            .bind(&finding.location)
            .bind(&finding.suggestion)
            .bind(&finding.example_excerpt)
            .bind(&finding.why_unsafe)
            .fetch_one(&mut *tx)
            .await
            .map_err(RepositoryError::Sqlx)?;

            inserted.push(map_finding_row(row)?);
        }

        tx.commit().await.map_err(RepositoryError::Sqlx)?;

        Ok(inserted)
    }

    pub async fn list_findings_for_scan(
        &self,
        scan_id: i64,
    ) -> Result<Vec<Finding>, RepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                scan_id,
                kind,
                title,
                category,
                severity,
                summary,
                location,
                suggestion,
                example_excerpt,
                why_unsafe
            FROM findings
            WHERE scan_id = $1
            ORDER BY id ASC
            "#,
        )
        .bind(scan_id)
        .fetch_all(&self.pool)
        .await
        .map_err(RepositoryError::Sqlx)?;

        rows.into_iter().map(map_finding_row).collect()
    }
}

#[derive(Debug)]
pub enum RepositoryError {
    InvalidEnumValue {
        field: &'static str,
        source: InvalidEnumValue,
    },
    Sqlx(sqlx::Error),
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidEnumValue { field, source } => {
                write!(f, "failed to decode field {field}: {source}")
            }
            Self::Sqlx(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RepositoryError {}

fn map_scan_row(row: PgRow) -> Result<Scan, RepositoryError> {
    Ok(Scan {
        id: row.try_get("id").map_err(RepositoryError::Sqlx)?,
        url: row.try_get("url").map_err(RepositoryError::Sqlx)?,
        normalized_url: row
            .try_get("normalized_url")
            .map_err(RepositoryError::Sqlx)?,
        status: parse_enum_field("status", &row)?,
        phase: parse_enum_field("phase", &row)?,
        accessibility_score: row
            .try_get("accessibility_score")
            .map_err(RepositoryError::Sqlx)?,
        inappropriate_score: row
            .try_get("inappropriate_score")
            .map_err(RepositoryError::Sqlx)?,
        risk_level: parse_optional_enum_field("risk_level", &row)?,
        content_safety_skipped: row
            .try_get("content_safety_skipped")
            .map_err(RepositoryError::Sqlx)?,
        error_reason: row.try_get("error_reason").map_err(RepositoryError::Sqlx)?,
        created_at: row.try_get("created_at").map_err(RepositoryError::Sqlx)?,
        updated_at: row.try_get("updated_at").map_err(RepositoryError::Sqlx)?,
    })
}

fn map_finding_row(row: PgRow) -> Result<Finding, RepositoryError> {
    Ok(Finding {
        id: row.try_get("id").map_err(RepositoryError::Sqlx)?,
        scan_id: row.try_get("scan_id").map_err(RepositoryError::Sqlx)?,
        kind: parse_enum_field("kind", &row)?,
        title: row.try_get("title").map_err(RepositoryError::Sqlx)?,
        category: parse_enum_field("category", &row)?,
        severity: parse_enum_field("severity", &row)?,
        summary: row.try_get("summary").map_err(RepositoryError::Sqlx)?,
        location: row.try_get("location").map_err(RepositoryError::Sqlx)?,
        suggestion: row.try_get("suggestion").map_err(RepositoryError::Sqlx)?,
        example_excerpt: row
            .try_get("example_excerpt")
            .map_err(RepositoryError::Sqlx)?,
        why_unsafe: row.try_get("why_unsafe").map_err(RepositoryError::Sqlx)?,
    })
}

fn parse_enum_field<T>(field: &'static str, row: &PgRow) -> Result<T, RepositoryError>
where
    T: FromStr<Err = InvalidEnumValue>,
{
    let value: String = row.try_get(field).map_err(RepositoryError::Sqlx)?;
    T::from_str(&value).map_err(|source| RepositoryError::InvalidEnumValue { field, source })
}

fn parse_optional_enum_field<T>(
    field: &'static str,
    row: &PgRow,
) -> Result<Option<T>, RepositoryError>
where
    T: FromStr<Err = InvalidEnumValue>,
{
    let value: Option<String> = row.try_get(field).map_err(RepositoryError::Sqlx)?;
    value
        .map(|value| {
            T::from_str(&value)
                .map_err(|source| RepositoryError::InvalidEnumValue { field, source })
        })
        .transpose()
}
