import { useState, type FormEvent, type ReactNode } from "react"

import {
  ArrowRight,
  BadgeAlert,
  CalendarClock,
  CircleAlert,
  DatabaseZap,
  EyeOff,
  Globe2,
  Link2,
  ListChecks,
  Radar,
  SearchCheck,
  ShieldAlert,
  ShieldCheck,
} from "lucide-react"

import { Button } from "@/components/ui/button"
import { ApiClientError } from "@/lib/api/client"
import { useCreateScan, useScan } from "@/lib/api/hooks"
import {
  isTerminalScanStatus,
  type CategoryBreakdownItem,
  type FindingDto,
  type FindingSeverity,
  type ScanErrorReason,
  type ScanPhase,
  type ScanResponse,
} from "@/lib/api/types"

const URL_PLACEHOLDER =
  "Enter a website URL, for example: https://example.com"
const INVALID_URL_MESSAGE = "Please enter a valid website URL."
const CONTENT_SAFETY_NOT_EVALUATED_MESSAGE =
  "Not evaluated — set `ANTHROPIC_API_KEY` to enable"

function App() {
  const [url, setUrl] = useState("")
  const [validationMessage, setValidationMessage] = useState<string | null>(null)
  const [activeScanId, setActiveScanId] = useState<number | null>(null)
  const createScan = useCreateScan()
  const activeScan = useScan(activeScanId)

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()

    const normalizedUrl = url.trim()
    const nextValidationMessage = validateWebsiteUrl(normalizedUrl)

    setValidationMessage(nextValidationMessage)

    if (nextValidationMessage !== null) {
      return
    }

    try {
      const response = await createScan.mutateAsync({ url: normalizedUrl })
      setActiveScanId(response.id)
    } catch (error) {
      if (error instanceof ApiClientError) {
        setValidationMessage(error.message)
        return
      }

      setValidationMessage("Unable to start the scan right now. Please try again.")
    }
  }

  async function handleRescan(nextUrl: string) {
    try {
      const response = await createScan.mutateAsync({
        url: nextUrl,
        force: true,
      })

      setUrl(nextUrl)
      setValidationMessage(null)
      setActiveScanId(response.id)
    } catch (error) {
      if (error instanceof ApiClientError) {
        setValidationMessage(error.message)
        return
      }

      setValidationMessage("Unable to start the scan right now. Please try again.")
    }
  }

  return (
    <main className="relative isolate min-h-screen overflow-hidden bg-[radial-gradient(circle_at_top_left,_rgba(167,243,208,0.26),_transparent_28%),radial-gradient(circle_at_top_right,_rgba(125,211,252,0.18),_transparent_26%),linear-gradient(180deg,_#f5fcf9_0%,_#e4f0eb_16%,_#bdd2cb_100%)] text-slate-950">
      <div className="absolute inset-0 bg-[linear-gradient(rgba(15,23,42,0.045)_1px,transparent_1px),linear-gradient(90deg,rgba(15,23,42,0.045)_1px,transparent_1px)] bg-[size:72px_72px] opacity-45" />
      <div className="absolute inset-x-0 top-[-6rem] h-80 bg-[radial-gradient(circle,_rgba(255,255,255,0.88)_0%,_transparent_68%)] blur-3xl" />
      <div className="relative mx-auto flex min-h-screen max-w-6xl flex-col px-6 py-8 sm:px-10 lg:px-12">
        <header className="flex flex-col gap-4 border-b border-slate-900/8 pb-5 sm:flex-row sm:items-end sm:justify-between">
          <div>
            <p className="font-mono text-xs uppercase tracking-[0.36em] text-emerald-950/55">
              ZeroClaw
            </p>
            <h1 className="mt-2 text-3xl font-semibold tracking-[-0.05em] text-slate-950 sm:text-4xl">
              Website Risk Scanner
            </h1>
            <p className="mt-3 max-w-2xl text-sm leading-6 text-slate-700 sm:text-base">
              Run a website review for accessibility and content-safety issues
              from one entry point.
            </p>
          </div>
          <div className="rounded-full border border-emerald-900/10 bg-white/70 px-3 py-1 font-mono text-xs text-emerald-950/72 shadow-[0_10px_30px_rgba(15,23,42,0.06)] backdrop-blur">
            Scan launcher ready
          </div>
        </header>

        <section className="py-10 sm:py-12">
          <form
            onSubmit={handleSubmit}
            className="rounded-[30px] border border-white/65 bg-white/74 p-4 shadow-[0_28px_80px_rgba(15,23,42,0.12)] backdrop-blur sm:p-6"
          >
            <div className="flex flex-col gap-5 lg:flex-row lg:items-end">
              <div className="min-w-0 flex-1">
                <label
                  htmlFor="website-url"
                  className="font-mono text-xs uppercase tracking-[0.28em] text-emerald-950/55"
                >
                  Website URL
                </label>
                <div className="mt-3 flex items-center gap-3 rounded-[24px] border border-slate-900/8 bg-white px-4 py-3 shadow-[inset_0_1px_0_rgba(255,255,255,0.55),0_10px_20px_rgba(15,23,42,0.04)]">
                  <Link2 className="size-4 shrink-0 text-emerald-900/45" />
                  <input
                    id="website-url"
                    name="website-url"
                    type="url"
                    inputMode="url"
                    autoComplete="off"
                    value={url}
                    onChange={(event) => {
                      setUrl(event.target.value)
                      if (validationMessage !== null) {
                        setValidationMessage(null)
                      }
                    }}
                    placeholder={URL_PLACEHOLDER}
                    aria-invalid={validationMessage !== null}
                    aria-describedby="website-url-hint website-url-error"
                    className="min-w-0 flex-1 bg-transparent text-sm text-slate-950 outline-none placeholder:text-slate-400 sm:text-base"
                  />
                </div>
                <p
                  id="website-url-hint"
                  className="mt-3 text-sm leading-6 text-slate-600"
                >
                  Public `http://` and `https://` addresses are supported.
                </p>
                <p
                  id="website-url-error"
                  className="mt-2 min-h-6 text-sm font-medium text-rose-700"
                >
                  {validationMessage}
                </p>
              </div>

              <Button
                type="submit"
                size="lg"
                disabled={createScan.isPending}
                className="h-13 rounded-[20px] bg-emerald-700 px-5 text-white shadow-[0_16px_40px_rgba(4,120,87,0.2)] hover:bg-emerald-600 disabled:bg-emerald-300/70"
              >
                {createScan.isPending ? "Submitting..." : "Analyze Website"}
                <ArrowRight className="size-4" />
              </Button>
            </div>
          </form>
        </section>

        <section className="grid flex-1 gap-6 pb-10 lg:grid-cols-[1.15fr_0.85fr]">
          <div className="space-y-6">
            {activeScanId === null ? (
              <EmptyState />
            ) : activeScan.data === undefined ? (
              <LoadingScanState
                id={activeScanId}
                phase="queued"
                isRefreshing={activeScan.isFetching || createScan.isPending}
              />
            ) : !isTerminalScanStatus(activeScan.data.status) ? (
              <LoadingScanState
                id={activeScan.data.id}
                phase={activeScan.data.phase}
                isRefreshing={activeScan.isFetching}
              />
            ) : (
              <TerminalScanState
                scan={activeScan.data}
                isRefreshing={activeScan.isFetching}
                isRescanning={createScan.isPending}
                onRescan={handleRescan}
                onTryAgain={() => {
                  setActiveScanId(null)
                  setValidationMessage(null)
                }}
              />
            )}
          </div>

          <div className="grid content-start gap-4">
            <StatusPanel />
            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-1">
              <InfoPanel
                title="What this step does"
                body="This screen collects the target URL, blocks obviously invalid client input, and kicks off a typed scan request."
              />
              <InfoPanel
                title="What comes next"
                body="Later issues will replace the simple active-scan card with phase-based loading, error handling, and a full results dashboard."
              />
            </div>
          </div>
        </section>
      </div>
    </main>
  )
}

export default App

type StatusCardProps = {
  icon: ReactNode
  label: string
  value: string
}

function StatusCard({ icon, label, value }: StatusCardProps) {
  return (
    <div className="rounded-[22px] border border-slate-900/8 bg-white/74 p-4 shadow-[0_14px_40px_rgba(15,23,42,0.08)]">
      <div className="mb-4 inline-flex rounded-xl border border-emerald-900/10 bg-emerald-50 p-2 text-emerald-800">
        {icon}
      </div>
      <p className="font-mono text-[11px] uppercase tracking-[0.28em] text-emerald-950/48">
        {label}
      </p>
      <p className="mt-2 text-lg font-semibold tracking-[-0.04em] text-slate-950">
        {value}
      </p>
    </div>
  )
}

type InfoPanelProps = {
  title: string
  body: string
}

function InfoPanel({ title, body }: InfoPanelProps) {
  return (
    <article className="rounded-[24px] border border-white/60 bg-white/72 p-5 shadow-[0_16px_44px_rgba(15,23,42,0.08)] backdrop-blur-sm">
      <p className="font-mono text-xs uppercase tracking-[0.28em] text-emerald-950/52">
        {title}
      </p>
      <p className="mt-3 text-sm leading-6 text-slate-700">{body}</p>
    </article>
  )
}

function EmptyState() {
  return (
    <article className="rounded-[32px] border border-dashed border-emerald-900/14 bg-white/72 p-8 shadow-[0_24px_80px_rgba(15,23,42,0.1)] backdrop-blur-sm sm:p-10">
      <div className="inline-flex rounded-2xl border border-emerald-900/12 bg-emerald-50 p-3 text-emerald-800">
        <SearchCheck className="size-6" />
      </div>
      <p className="mt-6 font-mono text-xs uppercase tracking-[0.32em] text-emerald-950/52">
        Empty state
      </p>
      <h2 className="mt-4 text-3xl font-semibold tracking-[-0.06em] text-slate-950 sm:text-4xl">
        No scan is active yet.
      </h2>
      <p className="mt-4 max-w-2xl text-base leading-7 text-slate-700">
        Enter a website URL above to start analyzing accessibility issues and
        inappropriate content risks. Your latest scan will take over this space
        once the request is submitted.
      </p>
    </article>
  )
}

type TerminalScanStateProps = {
  scan: ScanResponse
  isRefreshing: boolean
  isRescanning: boolean
  onRescan: (url: string) => Promise<void>
  onTryAgain: () => void
}

function TerminalScanState({
  scan,
  isRefreshing,
  isRescanning,
  onRescan,
  onTryAgain,
}: TerminalScanStateProps) {
  if (scan.status === "failed") {
    return (
      <ErrorScanState
        errorReason={scan.error_reason}
        isRefreshing={isRefreshing}
        onTryAgain={onTryAgain}
      />
    )
  }

  return (
    <Dashboard
      scan={scan}
      isRefreshing={isRefreshing}
      isRescanning={isRescanning}
      onRescan={onRescan}
    />
  )
}

type ErrorScanStateProps = {
  errorReason: ScanErrorReason | null
  isRefreshing: boolean
  onTryAgain: () => void
}

function ErrorScanState({
  errorReason,
  isRefreshing,
  onTryAgain,
}: ErrorScanStateProps) {
  return (
    <article className="rounded-[32px] border border-rose-200/45 bg-[linear-gradient(180deg,rgba(255,242,244,0.92)_0%,rgba(255,233,237,0.86)_100%)] p-8 shadow-[0_24px_80px_rgba(127,29,29,0.12)] backdrop-blur-sm sm:p-10">
      <div className="flex flex-col gap-5 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <div className="inline-flex rounded-2xl border border-rose-700/10 bg-rose-100 p-3 text-rose-700">
            <CircleAlert className="size-6" />
          </div>
          <p className="mt-6 font-mono text-xs uppercase tracking-[0.32em] text-rose-800/55">
            Scan failed
          </p>
          <h2 className="mt-4 text-3xl font-semibold tracking-[-0.06em] text-slate-950 sm:text-4xl">
            We could not scan this website. Please check the URL and try again.
          </h2>
          <p className="mt-4 max-w-2xl text-base leading-7 text-slate-700 sm:text-lg">
            {errorReasonMessageFor(errorReason)}
          </p>
        </div>
        <div className="rounded-full border border-rose-700/10 bg-white/75 px-3 py-1 font-mono text-xs uppercase tracking-[0.2em] text-rose-900/72">
          {isRefreshing ? "Refreshing" : "Stopped"}
        </div>
      </div>

      <div className="mt-8 flex flex-col gap-3 sm:flex-row">
        <Button
          type="button"
          size="lg"
          onClick={onTryAgain}
          className="bg-emerald-700 text-white hover:bg-emerald-600"
        >
          Try Again
          <ArrowRight className="size-4" />
        </Button>
      </div>
    </article>
  )
}

type LoadingScanStateProps = {
  id: number
  phase: ScanPhase
  isRefreshing: boolean
}

function LoadingScanState({
  id,
  phase,
  isRefreshing,
}: LoadingScanStateProps) {
  const phaseLine = phaseMessageFor(phase)

  return (
    <article
      data-testid="loading-screen"
      className="rounded-[32px] border border-white/60 bg-white/74 p-8 shadow-[0_24px_80px_rgba(15,23,42,0.1)] backdrop-blur-sm sm:p-10"
    >
      <div className="flex flex-col gap-5 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <p className="font-mono text-xs uppercase tracking-[0.32em] text-emerald-950/52">
            Scan in progress
          </p>
          <h2 className="mt-4 text-3xl font-semibold tracking-[-0.06em] text-slate-950 sm:text-4xl">
            Scanning website...
          </h2>
          <p className="mt-4 text-base leading-7 text-slate-700 sm:text-lg">
            {phaseLine}
          </p>
        </div>
        <div className="rounded-full border border-emerald-900/10 bg-emerald-50 px-3 py-1 font-mono text-xs uppercase tracking-[0.2em] text-emerald-950/70">
          {isRefreshing ? "Refreshing" : "Polling"}
        </div>
      </div>

      <div className="mt-8 grid gap-3 sm:grid-cols-3">
        <StatusCard
          icon={<Radar className="size-4" />}
          label="Scan"
          value={`#${id}`}
        />
        <StatusCard
          icon={<ShieldCheck className="size-4" />}
          label="Phase"
          value={formatEnumLabel(phase)}
        />
        <StatusCard
          icon={<DatabaseZap className="size-4" />}
          label="Progress"
          value="Working"
        />
      </div>
    </article>
  )
}

type DashboardProps = {
  scan: ScanResponse
  isRefreshing: boolean
  isRescanning: boolean
  onRescan: (url: string) => Promise<void>
}

function Dashboard({
  scan,
  isRefreshing,
  isRescanning,
  onRescan,
}: DashboardProps) {
  const totalIssues = scan.accessibility.length + scan.inappropriate.length
  const contentSafetySkipped = scan.content_safety_skipped

  return (
    <div data-testid="dashboard" className="grid gap-5 xl:gap-6">
      <WebsiteSummaryCard
        url={scan.url}
        createdAt={scan.created_at}
        riskLevel={scan.risk_level}
        totalIssues={totalIssues}
        isRefreshing={isRefreshing}
        isRescanning={isRescanning}
        onRescan={() => onRescan(scan.url)}
      />
      <div className="grid gap-4 xl:grid-cols-2">
        <ScoreCard
          title="Accessibility Score"
          value={scan.accessibility_score ?? 0}
          explanation="Counts the accessibility issues detected across the page."
          icon={<EyeOff className="size-5" />}
          tone={scoreToneFor(scan.accessibility_score ?? 0, false)}
          max={12}
          testId="accessibility-score-card"
        />
        <ScoreCard
          title="Inappropriate Score"
          value={contentSafetySkipped ? null : (scan.inappropriate_score ?? 0)}
          valueLabel={contentSafetySkipped ? "Not evaluated" : undefined}
          unitLabel={contentSafetySkipped ? "" : "issues"}
          explanation={
            contentSafetySkipped
              ? "Content safety was not evaluated for this scan."
              : "Weighted score based on inappropriate or unsafe content findings."
          }
          note={
            contentSafetySkipped
              ? CONTENT_SAFETY_NOT_EVALUATED_MESSAGE
              : undefined
          }
          icon={<BadgeAlert className="size-5" />}
          tone={
            contentSafetySkipped
              ? mutedScoreTone()
              : scoreToneFor(scan.inappropriate_score ?? 0, true)
          }
          max={16}
          testId="inappropriate-score-card"
          showMeter={!contentSafetySkipped}
        />
      </div>
      <div className="grid gap-4 xl:grid-cols-2">
        <FindingsSection
          title="Accessibility Findings"
          description="Detected accessibility issues with suggested fixes for the scanned page."
          icon={<ListChecks className="size-5" />}
          findings={scan.accessibility}
          emptyMessage="No accessibility findings were returned for this scan."
          testId="accessibility-findings"
          renderDetails={(finding) => (
            <div className="mt-4 grid gap-3">
              <FindingDetail
                label="Suggested fix"
                value={
                  finding.suggestion ??
                  "Review the affected element and apply the recommended accessibility improvement."
                }
              />
            </div>
          )}
        />
        <FindingsSection
          title="Inappropriate Content Findings"
          description="Unsafe or sensitive content findings with their category, excerpt, and recommended action."
          icon={<ShieldAlert className="size-5" />}
          findings={contentSafetySkipped ? [] : scan.inappropriate}
          emptyMessage={
            contentSafetySkipped
              ? CONTENT_SAFETY_NOT_EVALUATED_MESSAGE
              : "No inappropriate content findings were returned for this scan."
          }
          emptyVariant={contentSafetySkipped ? "info" : "default"}
          testId="inappropriate-findings"
          renderDetails={(finding) => (
            <div className="mt-4 grid gap-3">
              <FindingDetail
                label="Category"
                value={formatEnumLabel(finding.category)}
              />
              <FindingDetail
                label="Excerpt"
                value={
                  finding.example_excerpt ??
                  "No excerpt was provided for this finding."
                }
              />
              <FindingDetail
                label="Suggested action"
                value={
                  finding.suggestion ??
                  "Review the flagged content and apply the recommended moderation action."
                }
              />
            </div>
          )}
        />
      </div>
      <div className="grid gap-4 xl:grid-cols-[1.15fr_0.85fr]">
        <CategoryBreakdownCard
          breakdown={scan.category_breakdown ?? []}
          contentSafetySkipped={contentSafetySkipped}
        />
        <RecommendedActionsCard actions={scan.recommended_actions ?? []} />
      </div>
    </div>
  )
}

type WebsiteSummaryCardProps = {
  url: string
  createdAt: string
  riskLevel: ScanResponse["risk_level"]
  totalIssues: number
  isRefreshing: boolean
  isRescanning: boolean
  onRescan: () => void
}

function WebsiteSummaryCard({
  url,
  createdAt,
  riskLevel,
  totalIssues,
  isRefreshing,
  isRescanning,
  onRescan,
}: WebsiteSummaryCardProps) {
  const riskTone = riskToneFor(riskLevel)

  return (
    <article
      data-testid="dashboard-summary"
      className="rounded-[32px] border border-white/65 bg-[linear-gradient(135deg,rgba(255,255,255,0.86)_0%,rgba(241,251,247,0.9)_54%,rgba(226,244,238,0.95)_100%)] p-8 shadow-[0_28px_80px_rgba(15,23,42,0.12)] backdrop-blur-sm sm:p-10"
    >
      <div className="flex flex-col gap-5 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <p className="font-mono text-xs uppercase tracking-[0.32em] text-emerald-950/52">
            Dashboard
          </p>
          <h2 className="mt-3 text-3xl font-semibold tracking-[-0.07em] text-slate-950 sm:text-[2.65rem]">
            Scan complete.
          </h2>
          <div className="mt-5 inline-flex items-center gap-2 rounded-full border border-emerald-900/10 bg-white/78 px-3 py-1.5 text-sm text-slate-700 shadow-[0_12px_30px_rgba(15,23,42,0.06)]">
            <Globe2 className="size-4 shrink-0 text-emerald-700" />
            <span className="break-all">{url}</span>
          </div>
          <p className="mt-4 max-w-2xl text-sm leading-6 text-slate-600 sm:text-base">
            Snapshot of the current scan, issue totals, and next-step guidance
            from the completed analysis.
          </p>
        </div>
        <div className="rounded-full border border-emerald-900/10 bg-emerald-50 px-3 py-1 font-mono text-xs uppercase tracking-[0.2em] text-emerald-950/70">
          {isRefreshing ? "Refreshing" : "Complete"}
        </div>
      </div>

      <div className="mt-6 flex flex-col gap-3 sm:flex-row">
        <Button
          type="button"
          size="lg"
          onClick={onRescan}
          disabled={isRescanning}
          className="bg-emerald-700 text-white shadow-[0_18px_40px_rgba(4,120,87,0.18)] hover:bg-emerald-600 disabled:bg-emerald-300/70"
        >
          {isRescanning ? "Re-scanning..." : "Re-scan"}
          <ArrowRight className="size-4" />
        </Button>
      </div>

      <div className="mt-8 grid gap-4 md:grid-cols-[1.2fr_0.8fr]">
        <div className="rounded-[24px] border border-slate-900/8 bg-white/80 p-5 shadow-[0_16px_40px_rgba(15,23,42,0.06)]">
          <div className="flex items-center gap-2 font-mono text-[11px] uppercase tracking-[0.28em] text-emerald-950/48">
            <CalendarClock className="size-4 text-emerald-700/80" />
            <span>Scan timestamp</span>
          </div>
          <p className="mt-3 text-xl font-semibold tracking-[-0.04em] text-slate-950">
            {formatScanTimestamp(createdAt)}
          </p>
        </div>

        <div className="grid gap-4 sm:grid-cols-2 md:grid-cols-1">
          <div className={`rounded-[24px] border p-5 shadow-[0_16px_40px_rgba(15,23,42,0.06)] ${riskTone.panelClass}`}>
            <p className="font-mono text-[11px] uppercase tracking-[0.28em] text-slate-900/48">
              Risk level
            </p>
            <div
              data-testid="risk-level-badge"
              className={`mt-3 inline-flex rounded-full border px-3 py-1 font-mono text-xs uppercase tracking-[0.2em] ${riskTone.badgeClass}`}
            >
              {riskLevel === null ? "Unavailable" : riskLevel}
            </div>
          </div>

          <div className="rounded-[24px] border border-slate-900/8 bg-white/80 p-5 shadow-[0_16px_40px_rgba(15,23,42,0.06)]">
            <p className="font-mono text-[11px] uppercase tracking-[0.28em] text-emerald-950/48">
              Total issues
            </p>
            <p className="mt-2 text-4xl font-semibold tracking-[-0.08em] text-slate-950">
              {totalIssues}
            </p>
            <p className="mt-2 text-sm text-slate-600">
              Combined accessibility and content-safety findings from this pass.
            </p>
          </div>
        </div>
      </div>
    </article>
  )
}

type ScoreCardProps = {
  title: string
  value: number | null
  valueLabel?: string
  unitLabel?: string
  explanation: string
  note?: string
  icon: ReactNode
  tone: ScoreTone
  max: number
  testId: string
  showMeter?: boolean
}

function ScoreCard({
  title,
  value,
  valueLabel,
  unitLabel = "issues",
  explanation,
  note,
  icon,
  tone,
  max,
  testId,
  showMeter = true,
}: ScoreCardProps) {
  const percentage =
    value === null ? 0 : Math.min(100, Math.round((value / max) * 100))

  return (
    <article
      data-testid={testId}
      className={`rounded-[28px] border p-6 shadow-[0_24px_70px_rgba(15,23,42,0.08)] backdrop-blur-sm ${tone.panelClass}`}
    >
      <div className="flex items-start justify-between gap-4">
        <div className="space-y-3">
          <p className="font-mono text-xs uppercase tracking-[0.28em] text-slate-900/52">
            {title}
          </p>
          <div className="flex items-end gap-3">
            <p
              data-testid={`${testId}-value`}
              className={
                valueLabel === undefined
                  ? "text-5xl font-semibold tracking-[-0.09em] text-slate-950"
                  : "max-w-[12rem] text-3xl font-semibold leading-none tracking-[-0.07em] text-slate-950"
              }
            >
              {valueLabel ?? value}
            </p>
            {unitLabel.length > 0 ? (
              <span className="pb-1 font-mono text-[11px] uppercase tracking-[0.22em] text-slate-900/42">
                {unitLabel}
              </span>
            ) : null}
          </div>
        </div>
        <div className={`rounded-[20px] border p-3 shadow-[inset_0_1px_0_rgba(255,255,255,0.35)] ${tone.iconClass}`}>
          {icon}
        </div>
      </div>

      <p className="mt-4 text-sm leading-6 text-slate-700">{explanation}</p>
      {note === undefined ? null : (
        <p className="mt-3 rounded-2xl border border-slate-900/8 bg-white/60 px-4 py-3 text-sm leading-6 text-slate-600">
          {note}
        </p>
      )}

      {showMeter ? (
        <div className="mt-6">
          <div className="mb-2.5 flex items-center justify-between font-mono text-[11px] uppercase tracking-[0.24em] text-slate-900/45">
            <span>Relative severity</span>
            <span>{percentage}%</span>
          </div>
          <div className="h-3 overflow-hidden rounded-full bg-slate-900/8">
            <div
              className={`h-full rounded-full transition-[width] duration-500 ${tone.barClass}`}
              style={{ width: `${Math.max(8, percentage)}%` }}
            />
          </div>
        </div>
      ) : null}
    </article>
  )
}

type FindingsSectionProps = {
  title: string
  description: string
  icon: ReactNode
  findings: FindingDto[]
  emptyMessage: string
  emptyVariant?: "default" | "info"
  renderDetails: (finding: FindingDto) => ReactNode
  testId: string
}

function FindingsSection({
  title,
  description,
  icon,
  findings,
  emptyMessage,
  emptyVariant = "default",
  renderDetails,
  testId,
}: FindingsSectionProps) {
  return (
    <section
      data-testid={testId}
      className="rounded-[28px] border border-white/60 bg-white/74 p-6 shadow-[0_24px_70px_rgba(15,23,42,0.08)] backdrop-blur-sm"
    >
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="font-mono text-xs uppercase tracking-[0.28em] text-emerald-950/52">
            {title}
          </p>
          <p className="mt-3 max-w-xl text-sm leading-6 text-slate-700">
            {description}
          </p>
        </div>
        <div className="rounded-2xl border border-emerald-900/10 bg-emerald-50 p-3 text-emerald-800 shadow-[0_10px_30px_rgba(15,23,42,0.05)]">
          {icon}
        </div>
      </div>

      {findings.length === 0 ? (
        <div
          className={`mt-6 rounded-[24px] border border-dashed p-5 text-sm leading-6 ${
            emptyVariant === "info"
              ? "border-slate-900/12 bg-slate-100/70 text-slate-600"
              : "border-slate-900/10 bg-slate-50/70 text-slate-600"
          }`}
        >
          {emptyMessage}
        </div>
      ) : (
        <div className="mt-6 space-y-4">
          {findings.map((finding) => (
            <article
              key={finding.id}
              className="rounded-[24px] border border-slate-900/8 bg-[linear-gradient(180deg,rgba(255,255,255,0.9)_0%,rgba(245,250,248,0.94)_100%)] p-5 shadow-[0_14px_32px_rgba(15,23,42,0.05)]"
            >
              <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
                <div className="min-w-0">
                  <h3 className="text-lg font-semibold tracking-[-0.04em] text-slate-950">
                    {finding.title}
                  </h3>
                  <p className="mt-2.5 text-sm leading-6 text-slate-700">
                    {finding.summary}
                  </p>
                </div>
                <SeverityBadge severity={finding.severity} />
              </div>

              {renderDetails(finding)}
            </article>
          ))}
        </div>
      )}
    </section>
  )
}

type CategoryBreakdownCardProps = {
  breakdown: CategoryBreakdownItem[]
  contentSafetySkipped: boolean
}

function CategoryBreakdownCard({
  breakdown,
  contentSafetySkipped,
}: CategoryBreakdownCardProps) {
  return (
    <section
      data-testid="category-breakdown"
      className="rounded-[28px] border border-white/60 bg-white/74 p-6 shadow-[0_24px_70px_rgba(15,23,42,0.08)] backdrop-blur-sm"
    >
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="font-mono text-xs uppercase tracking-[0.28em] text-emerald-950/52">
            Content Analysis Breakdown
          </p>
          <p className="mt-3 max-w-xl text-sm leading-6 text-slate-700">
            Category counts from the completed scan response, including
            zero-count rows returned by the server.
          </p>
        </div>
        <div className="rounded-2xl border border-emerald-900/10 bg-emerald-50 p-3 text-emerald-800 shadow-[0_10px_30px_rgba(15,23,42,0.05)]">
          <BadgeAlert className="size-5" />
        </div>
      </div>

      {contentSafetySkipped ? (
        <div className="mt-6 rounded-[24px] border border-dashed border-slate-900/12 bg-slate-100/70 p-5 text-sm leading-6 text-slate-600">
          {CONTENT_SAFETY_NOT_EVALUATED_MESSAGE}
        </div>
      ) : (
        <div className="mt-6 overflow-hidden rounded-[24px] border border-slate-900/8 bg-[linear-gradient(180deg,rgba(255,255,255,0.92)_0%,rgba(244,248,247,0.95)_100%)] shadow-[inset_0_1px_0_rgba(255,255,255,0.4)]">
          <div className="grid grid-cols-[1fr_auto] gap-3 border-b border-slate-900/8 px-4 py-3 font-mono text-[11px] uppercase tracking-[0.24em] text-slate-900/48">
            <span>Category</span>
            <span>Count</span>
          </div>
          <div className="divide-y divide-slate-900/6">
            {breakdown.map((item) => (
              <div
                key={item.category}
                className="grid grid-cols-[1fr_auto] gap-3 px-4 py-3.5 text-sm text-slate-700"
              >
                <span>{formatEnumLabel(item.category)}</span>
                <span className="font-mono text-emerald-950/78">
                  {item.count}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </section>
  )
}

type RecommendedActionsCardProps = {
  actions: string[]
}

function RecommendedActionsCard({ actions }: RecommendedActionsCardProps) {
  return (
    <section
      data-testid="recommended-actions"
      className="rounded-[28px] border border-white/60 bg-white/74 p-6 shadow-[0_24px_70px_rgba(15,23,42,0.08)] backdrop-blur-sm"
    >
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="font-mono text-xs uppercase tracking-[0.28em] text-emerald-950/52">
            Recommended Actions
          </p>
          <p className="mt-3 max-w-xl text-sm leading-6 text-slate-700">
            Remediation lines supplied by the server for the current mix of
            findings.
          </p>
        </div>
        <div className="rounded-2xl border border-emerald-900/10 bg-emerald-50 p-3 text-emerald-800 shadow-[0_10px_30px_rgba(15,23,42,0.05)]">
          <ShieldCheck className="size-5" />
        </div>
      </div>

      {actions.length === 0 ? (
        <div className="mt-6 rounded-[24px] border border-dashed border-slate-900/10 bg-slate-50/70 p-5 text-sm leading-6 text-slate-600">
          No recommended actions were returned for this scan.
        </div>
      ) : (
        <ol className="mt-6 space-y-3">
          {actions.map((action, index) => (
            <li
              key={`${index}-${action}`}
              className="flex gap-3 rounded-[22px] border border-slate-900/8 bg-[linear-gradient(180deg,rgba(255,255,255,0.9)_0%,rgba(245,250,248,0.94)_100%)] p-4 shadow-[0_14px_30px_rgba(15,23,42,0.05)]"
            >
              <span className="mt-0.5 inline-flex size-6 shrink-0 items-center justify-center rounded-full border border-emerald-900/10 bg-emerald-50 font-mono text-[11px] text-emerald-900/82">
                {index + 1}
              </span>
              <p className="text-sm leading-6 text-slate-700">{action}</p>
            </li>
          ))}
        </ol>
      )}
    </section>
  )
}

type FindingDetailProps = {
  label: string
  value: string
}

function FindingDetail({ label, value }: FindingDetailProps) {
  return (
    <div className="rounded-[18px] border border-slate-900/8 bg-slate-50/72 p-4">
      <p className="font-mono text-[11px] uppercase tracking-[0.24em] text-slate-900/45">
        {label}
      </p>
      <p className="mt-2 text-sm leading-6 text-slate-700">{value}</p>
    </div>
  )
}

type SeverityBadgeProps = {
  severity: FindingSeverity
}

function SeverityBadge({ severity }: SeverityBadgeProps) {
  const tone = severityToneFor(severity)

  return (
    <span
      className={`inline-flex shrink-0 rounded-full border px-3 py-1 font-mono text-[11px] uppercase tracking-[0.22em] shadow-[inset_0_1px_0_rgba(255,255,255,0.2)] ${tone}`}
    >
      {severity}
    </span>
  )
}

function StatusPanel() {
  return (
    <article className="rounded-[28px] border border-white/60 bg-white/72 p-6 shadow-[0_24px_70px_rgba(15,23,42,0.1)] backdrop-blur-sm">
      <div className="mb-8 flex items-start justify-between">
        <div>
          <p className="font-mono text-xs uppercase tracking-[0.3em] text-emerald-950/58">
            Scan setup
          </p>
          <p className="mt-3 text-2xl font-semibold tracking-[-0.05em] text-slate-950">
            Front door wired
          </p>
        </div>
        <div className="rounded-2xl border border-emerald-900/12 bg-emerald-50 p-3 text-emerald-800">
          <DatabaseZap className="size-6" />
        </div>
      </div>

      <div className="grid gap-3 sm:grid-cols-3 lg:grid-cols-1 xl:grid-cols-3">
        <StatusCard
          icon={<Link2 className="size-4" />}
          label="Input"
          value="Validated"
        />
        <StatusCard
          icon={<SearchCheck className="size-4" />}
          label="Empty state"
          value="Ready"
        />
        <StatusCard
          icon={<ArrowRight className="size-4" />}
          label="Next"
          value="Loading UI"
        />
      </div>
    </article>
  )
}

function validateWebsiteUrl(value: string) {
  if (value.length === 0) {
    return INVALID_URL_MESSAGE
  }

  try {
    const url = new URL(value)

    if (
      (url.protocol !== "http:" && url.protocol !== "https:") ||
      url.hostname.length === 0
    ) {
      return INVALID_URL_MESSAGE
    }

    return null
  } catch {
    return INVALID_URL_MESSAGE
  }
}

function formatEnumLabel(value: string) {
  return value
    .split("_")
    .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
    .join(" ")
}

function formatScanTimestamp(value: string) {
  const parsed = new Date(value)

  if (Number.isNaN(parsed.getTime())) {
    return value
  }

  return new Intl.DateTimeFormat("en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(parsed)
}

function phaseMessageFor(phase: ScanPhase) {
  switch (phase) {
    case "accessibility":
      return "Checking accessibility..."
    case "content_safety":
      return "Reviewing content safety..."
    case "aggregating":
      return "Generating dashboard..."
    case "queued":
    case "loading":
    case "completed":
    case "failed":
      return "Scanning website..."
  }
}

function errorReasonMessageFor(errorReason: ScanErrorReason | null) {
  switch (errorReason) {
    case "invalid_url":
      return "The website address was not accepted for scanning. Please confirm the URL and submit it again."
    case "unreachable":
      return "The website could not be reached from the scanner. Make sure it is publicly available and try again."
    case "blocked":
      return "The website blocked automated access before the scan could finish. Try again or test a different public page."
    case "timeout":
      return "The website took too long to respond and the scan timed out. Please try again in a moment."
    case "no_content":
      return "The website loaded without enough readable page content to analyze. Please verify the page and try again."
    case null:
      return "The scan stopped before results were produced. Please check the URL and try again."
    default:
      return "The scanner hit an unexpected website error. Please check the URL and try again."
  }
}

type ScoreTone = {
  panelClass: string
  iconClass: string
  barClass: string
}

function scoreToneFor(value: number, weighted: boolean): ScoreTone {
  const mediumThreshold = weighted ? 5 : 3
  const highThreshold = weighted ? 10 : 7

  if (value >= highThreshold) {
    return {
      panelClass:
        "border-rose-300/40 bg-[linear-gradient(180deg,rgba(255,242,244,0.92)_0%,rgba(255,228,233,0.88)_100%)]",
      iconClass: "border-rose-700/10 bg-rose-100 text-rose-700",
      barClass: "bg-rose-600",
    }
  }

  if (value >= mediumThreshold) {
    return {
      panelClass:
        "border-amber-300/40 bg-[linear-gradient(180deg,rgba(255,249,235,0.94)_0%,rgba(254,240,199,0.88)_100%)]",
      iconClass: "border-amber-700/10 bg-amber-100 text-amber-700",
      barClass: "bg-amber-500",
    }
  }

  return {
    panelClass:
      "border-emerald-300/40 bg-[linear-gradient(180deg,rgba(236,253,245,0.94)_0%,rgba(209,250,229,0.88)_100%)]",
    iconClass: "border-emerald-700/10 bg-emerald-100 text-emerald-700",
    barClass: "bg-emerald-600",
  }
}

function mutedScoreTone(): ScoreTone {
  return {
    panelClass:
      "border-slate-300/45 bg-[linear-gradient(180deg,rgba(248,250,252,0.94)_0%,rgba(226,232,240,0.84)_100%)]",
    iconClass: "border-slate-700/10 bg-slate-100 text-slate-600",
    barClass: "bg-slate-400",
  }
}

function riskToneFor(riskLevel: ScanResponse["risk_level"]) {
  switch (riskLevel) {
    case "critical":
      return {
        panelClass: "border-fuchsia-300/45 bg-fuchsia-50/82",
        badgeClass: "border-fuchsia-700/15 bg-fuchsia-100 text-fuchsia-900",
      }
    case "high":
      return {
        panelClass: "border-rose-300/45 bg-rose-50/82",
        badgeClass: "border-rose-700/15 bg-rose-100 text-rose-900",
      }
    case "medium":
      return {
        panelClass: "border-amber-300/45 bg-amber-50/82",
        badgeClass: "border-amber-700/15 bg-amber-100 text-amber-900",
      }
    case "low":
      return {
        panelClass: "border-emerald-300/45 bg-emerald-50/82",
        badgeClass: "border-emerald-700/15 bg-emerald-100 text-emerald-900",
      }
    case null:
      return {
        panelClass: "border-slate-300/45 bg-white/78",
        badgeClass: "border-slate-400/20 bg-slate-100 text-slate-800",
      }
  }
}

function severityToneFor(severity: FindingSeverity) {
  switch (severity) {
    case "critical":
      return "border-fuchsia-700/15 bg-fuchsia-100 text-fuchsia-950"
    case "high":
      return "border-rose-700/15 bg-rose-100 text-rose-950"
    case "medium":
      return "border-amber-700/15 bg-amber-100 text-amber-950"
    case "low":
      return "border-emerald-700/15 bg-emerald-100 text-emerald-950"
  }
}
