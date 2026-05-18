import { useState, type FormEvent, type ReactNode } from "react"

import {
  ArrowRight,
  DatabaseZap,
  Link2,
  Radar,
  SearchCheck,
  ShieldCheck,
} from "lucide-react"

import { Button } from "@/components/ui/button"
import { ApiClientError } from "@/lib/api/client"
import { useCreateScan, useScan } from "@/lib/api/hooks"

const URL_PLACEHOLDER =
  "Enter a website URL, for example: https://example.com"
const INVALID_URL_MESSAGE = "Please enter a valid website URL."

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
      }
    }
  }

  return (
    <main className="relative isolate min-h-screen overflow-hidden bg-[radial-gradient(circle_at_top,_rgba(39,144,110,0.22),_transparent_36%),linear-gradient(180deg,_#081310_0%,_#071f19_50%,_#04100d_100%)] text-foreground">
      <div className="absolute inset-0 bg-[linear-gradient(rgba(217,239,229,0.06)_1px,transparent_1px),linear-gradient(90deg,rgba(217,239,229,0.06)_1px,transparent_1px)] bg-[size:72px_72px] opacity-20" />
      <div className="relative mx-auto flex min-h-screen max-w-6xl flex-col px-6 py-8 sm:px-10 lg:px-12">
        <header className="flex flex-col gap-4 border-b border-white/10 pb-5 sm:flex-row sm:items-end sm:justify-between">
          <div>
            <p className="font-mono text-xs uppercase tracking-[0.36em] text-emerald-200/70">
              ZeroClaw
            </p>
            <h1 className="mt-2 text-2xl font-semibold tracking-[-0.04em] text-white sm:text-3xl">
              Website Risk Scanner
            </h1>
            <p className="mt-3 max-w-2xl text-sm leading-6 text-emerald-50/70 sm:text-base">
              Run a website review for accessibility and content-safety issues
              from one entry point.
            </p>
          </div>
          <div className="rounded-full border border-emerald-300/20 bg-white/6 px-3 py-1 font-mono text-xs text-emerald-100/80 backdrop-blur">
            Scan launcher ready
          </div>
        </header>

        <section className="py-10 sm:py-12">
          <form
            onSubmit={handleSubmit}
            className="rounded-[30px] border border-white/12 bg-black/22 p-4 shadow-[0_28px_80px_rgba(0,0,0,0.32)] backdrop-blur sm:p-6"
          >
            <div className="flex flex-col gap-5 lg:flex-row lg:items-end">
              <div className="min-w-0 flex-1">
                <label
                  htmlFor="website-url"
                  className="font-mono text-xs uppercase tracking-[0.28em] text-emerald-200/65"
                >
                  Website URL
                </label>
                <div className="mt-3 flex items-center gap-3 rounded-[24px] border border-white/12 bg-white/8 px-4 py-3 shadow-[inset_0_1px_0_rgba(255,255,255,0.05)]">
                  <Link2 className="size-4 shrink-0 text-emerald-200/65" />
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
                    className="min-w-0 flex-1 bg-transparent text-sm text-white outline-none placeholder:text-emerald-50/38 sm:text-base"
                  />
                </div>
                <p
                  id="website-url-hint"
                  className="mt-3 text-sm leading-6 text-emerald-50/58"
                >
                  Public `http://` and `https://` addresses are supported.
                </p>
                <p
                  id="website-url-error"
                  className="mt-2 min-h-6 text-sm font-medium text-rose-200"
                >
                  {validationMessage}
                </p>
              </div>

              <Button
                type="submit"
                size="lg"
                disabled={createScan.isPending}
                className="h-13 rounded-[20px] bg-emerald-300 px-5 text-emerald-950 hover:bg-emerald-200 disabled:bg-emerald-100/60"
              >
                {createScan.isPending ? "Submitting..." : "Analyze Website"}
                <ArrowRight className="size-4" />
              </Button>
            </div>
          </form>
        </section>

        <section className="grid flex-1 gap-6 pb-10 lg:grid-cols-[1.15fr_0.85fr]">
          <div className="space-y-6">
            {activeScan.data === undefined ? (
              <EmptyState />
            ) : (
              <ActiveScanState
                id={activeScan.data.id}
                status={activeScan.data.status}
                phase={activeScan.data.phase}
                isRefreshing={activeScan.isFetching}
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
    <div className="rounded-2xl border border-white/10 bg-black/18 p-4">
      <div className="mb-4 inline-flex rounded-xl border border-white/10 bg-white/8 p-2 text-emerald-100">
        {icon}
      </div>
      <p className="font-mono text-[11px] uppercase tracking-[0.28em] text-emerald-100/55">
        {label}
      </p>
      <p className="mt-2 text-lg font-semibold tracking-[-0.04em] text-white">
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
    <article className="rounded-[24px] border border-white/10 bg-white/6 p-5 backdrop-blur-sm">
      <p className="font-mono text-xs uppercase tracking-[0.28em] text-emerald-200/60">
        {title}
      </p>
      <p className="mt-3 text-sm leading-6 text-emerald-50/75">{body}</p>
    </article>
  )
}

function EmptyState() {
  return (
    <article className="rounded-[32px] border border-dashed border-white/14 bg-white/5 p-8 shadow-[0_24px_80px_rgba(0,0,0,0.24)] backdrop-blur-sm sm:p-10">
      <div className="inline-flex rounded-2xl border border-emerald-200/15 bg-emerald-300/12 p-3 text-emerald-100">
        <SearchCheck className="size-6" />
      </div>
      <p className="mt-6 font-mono text-xs uppercase tracking-[0.32em] text-emerald-200/62">
        Empty state
      </p>
      <h2 className="mt-4 text-3xl font-semibold tracking-[-0.06em] text-white sm:text-4xl">
        No scan is active yet.
      </h2>
      <p className="mt-4 max-w-2xl text-base leading-7 text-emerald-50/72">
        Enter a website URL above to start analyzing accessibility issues and
        inappropriate content risks. Your latest scan will take over this space
        once the request is submitted.
      </p>
    </article>
  )
}

type ActiveScanStateProps = {
  id: number
  status: string
  phase: string
  isRefreshing: boolean
}

function ActiveScanState({
  id,
  status,
  phase,
  isRefreshing,
}: ActiveScanStateProps) {
  return (
    <article className="rounded-[32px] border border-white/12 bg-white/7 p-8 shadow-[0_24px_80px_rgba(0,0,0,0.24)] backdrop-blur-sm sm:p-10">
      <div className="flex flex-col gap-5 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <p className="font-mono text-xs uppercase tracking-[0.32em] text-emerald-200/62">
            Active scan
          </p>
          <h2 className="mt-4 text-3xl font-semibold tracking-[-0.06em] text-white sm:text-4xl">
            Scan #{id} is in progress.
          </h2>
        </div>
        <div className="rounded-full border border-emerald-200/14 bg-emerald-300/10 px-3 py-1 font-mono text-xs uppercase tracking-[0.2em] text-emerald-100/80">
          {isRefreshing ? "Refreshing" : "Live"}
        </div>
      </div>

      <div className="mt-8 grid gap-3 sm:grid-cols-2">
        <StatusCard
          icon={<Radar className="size-4" />}
          label="Status"
          value={formatEnumLabel(status)}
        />
        <StatusCard
          icon={<ShieldCheck className="size-4" />}
          label="Phase"
          value={formatEnumLabel(phase)}
        />
      </div>
    </article>
  )
}

function StatusPanel() {
  return (
    <article className="rounded-[28px] border border-white/12 bg-white/8 p-6 shadow-[0_24px_80px_rgba(0,0,0,0.28)] backdrop-blur-sm">
      <div className="mb-10 flex items-start justify-between">
        <div>
          <p className="font-mono text-xs uppercase tracking-[0.3em] text-emerald-200/70">
            Scan setup
          </p>
          <p className="mt-3 text-2xl font-semibold tracking-[-0.05em] text-white">
            Front door wired
          </p>
        </div>
        <div className="rounded-2xl border border-emerald-200/15 bg-emerald-300/14 p-3 text-emerald-100">
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
