CREATE TABLE scans (
    id BIGSERIAL PRIMARY KEY,
    url TEXT NOT NULL,
    normalized_url TEXT NOT NULL,
    status TEXT NOT NULL,
    phase TEXT NOT NULL,
    accessibility_score INTEGER,
    inappropriate_score INTEGER,
    risk_level TEXT,
    error_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE findings (
    id BIGSERIAL PRIMARY KEY,
    scan_id BIGINT NOT NULL REFERENCES scans(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    category TEXT NOT NULL,
    severity TEXT NOT NULL,
    summary TEXT NOT NULL,
    location TEXT,
    suggestion TEXT,
    example_excerpt TEXT,
    why_unsafe TEXT
);

CREATE INDEX idx_scans_status ON scans(status);
CREATE INDEX idx_scans_phase ON scans(phase);
CREATE INDEX idx_scans_created_at ON scans(created_at DESC);
CREATE INDEX idx_scans_normalized_url ON scans(normalized_url);

CREATE INDEX idx_findings_scan_id ON findings(scan_id);
CREATE INDEX idx_findings_kind ON findings(kind);
CREATE INDEX idx_findings_category ON findings(category);
CREATE INDEX idx_findings_severity ON findings(severity);
CREATE INDEX idx_findings_scan_kind ON findings(scan_id, kind);
