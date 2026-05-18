import type { ReactNode } from "react"

import { ArrowRight, DatabaseZap, SearchCheck, ShieldCheck } from "lucide-react"

import { Button } from "@/components/ui/button"
import { scanQueryKeys } from "@/lib/api/hooks"

function App() {
  return (
    <main className="relative isolate min-h-screen overflow-hidden bg-[radial-gradient(circle_at_top,_rgba(39,144,110,0.22),_transparent_36%),linear-gradient(180deg,_#081310_0%,_#071f19_50%,_#04100d_100%)] text-foreground">
      <div className="absolute inset-0 bg-[linear-gradient(rgba(217,239,229,0.06)_1px,transparent_1px),linear-gradient(90deg,rgba(217,239,229,0.06)_1px,transparent_1px)] bg-[size:72px_72px] opacity-20" />
      <div className="relative mx-auto flex min-h-screen max-w-6xl flex-col px-6 py-8 sm:px-10 lg:px-12">
        <header className="flex items-center justify-between border-b border-white/10 pb-5">
          <div>
            <p className="font-mono text-xs uppercase tracking-[0.36em] text-emerald-200/70">
              ZeroClaw
            </p>
            <h1 className="mt-2 text-xl font-semibold tracking-[-0.04em] text-white sm:text-2xl">
              Website Risk Scanner
            </h1>
          </div>
          <div className="rounded-full border border-emerald-300/20 bg-white/6 px-3 py-1 font-mono text-xs text-emerald-100/80 backdrop-blur">
            React + Vite + Tailwind + shadcn/ui
          </div>
        </header>

        <section className="grid flex-1 items-center gap-16 py-12 lg:grid-cols-[1.1fr_0.9fr] lg:py-18">
          <div className="max-w-2xl">
            <p className="font-mono text-sm uppercase tracking-[0.4em] text-emerald-200/70">
              Frontend bootstrap complete
            </p>
            <h2 className="mt-5 text-5xl font-semibold tracking-[-0.08em] text-white sm:text-6xl">
              Scan websites for accessibility and content risk from one dashboard.
            </h2>
            <p className="mt-6 max-w-xl text-base leading-7 text-emerald-50/72 sm:text-lg">
              This placeholder landing page proves the React app shell, TanStack
              Query provider, Tailwind styling, lucide icons, and shadcn/ui
              components are wired and ready for the scan workflow.
            </p>

            <div className="mt-8 flex flex-col gap-3 sm:flex-row">
              <Button
                size="lg"
                className="bg-emerald-300 text-emerald-950 hover:bg-emerald-200"
              >
                Start building the scan flow
                <ArrowRight className="size-4" />
              </Button>
              <Button
                variant="outline"
                size="lg"
                className="border-white/15 bg-white/8 text-white hover:bg-white/14"
              >
                API contract arrives in issue #21
              </Button>
            </div>
          </div>

          <div className="grid gap-4">
            <article className="rounded-[28px] border border-white/12 bg-white/8 p-6 shadow-[0_24px_80px_rgba(0,0,0,0.28)] backdrop-blur-sm">
              <div className="mb-10 flex items-start justify-between">
                <div>
                  <p className="font-mono text-xs uppercase tracking-[0.3em] text-emerald-200/70">
                    Placeholder dashboard
                  </p>
                  <p className="mt-3 text-2xl font-semibold tracking-[-0.05em] text-white">
                    Frontend foundation ready
                  </p>
                </div>
                <div className="rounded-2xl border border-emerald-200/15 bg-emerald-300/14 p-3 text-emerald-100">
                  <SearchCheck className="size-6" />
                </div>
              </div>

              <div className="grid gap-3 sm:grid-cols-3">
                <StatusCard
                  icon={<ShieldCheck className="size-4" />}
                  label="UI system"
                  value="shadcn"
                />
                <StatusCard
                  icon={<DatabaseZap className="size-4" />}
                  label="Data layer"
                  value="Query"
                />
                <StatusCard
                  icon={<ArrowRight className="size-4" />}
                  label="Next milestone"
                  value="API client"
                />
              </div>
            </article>

            <div className="grid gap-4 sm:grid-cols-2">
              <InfoPanel
                title="What landed"
                body="Vite TypeScript app, Tailwind v4, shadcn/ui init, lucide-react, TanStack Query, and a typed API client layer for scans."
              />
              <InfoPanel
                title="API contract"
                body={`Query keys live under ${scanQueryKeys.all[0]}, with typed POST /api/scans and GET /api/scans/:id hooks ready for the next UI issues.`}
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
