export type ScanStatus = "pending" | "running" | "completed" | "failed"

export type ScanPhase =
  | "queued"
  | "loading"
  | "accessibility"
  | "content_safety"
  | "aggregating"
  | "completed"
  | "failed"

export type RiskLevel = "low" | "medium" | "high" | "critical"

export type FindingCategory =
  | "accessibility"
  | "adult_content"
  | "alcohol"
  | "drugs"
  | "gambling"
  | "hate"
  | "self_harm"
  | "sexual_content"
  | "violence"
  | "weapons"
  | "illegal_activity"
  | "harassment"
  | "extremism"
  | "deceptive_practices"

export type FindingSeverity = "low" | "medium" | "high" | "critical"

export type CreateScanRequest = {
  url: string
  force?: boolean
}

export type CreateScanResponse = {
  id: number
  cached: boolean
}

export type FindingDto = {
  id: number
  title: string
  category: FindingCategory
  severity: FindingSeverity
  summary: string
  location: string | null
  suggestion: string | null
  example_excerpt: string | null
  why_unsafe: string | null
}

export type CategoryBreakdownItem = {
  category: FindingCategory
  count: number
}

export type ScanResponse = {
  id: number
  status: ScanStatus
  phase: ScanPhase
  accessibility_score: number | null
  inappropriate_score: number | null
  risk_level: RiskLevel | null
  error_reason: string | null
  accessibility: FindingDto[]
  inappropriate: FindingDto[]
  category_breakdown: CategoryBreakdownItem[] | null
}

export type ErrorResponse = {
  error: string
}

export function isTerminalScanStatus(status: ScanStatus) {
  return status === "completed" || status === "failed"
}
