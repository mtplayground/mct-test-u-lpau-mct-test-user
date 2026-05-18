# Product Snapshot

## What This Project Is

ZeroClaw is a website risk scanner with a Rust backend and React frontend. It
analyzes a submitted public URL for two things:

- accessibility issues
- inappropriate or unsafe content signals

It stores scan state and findings in PostgreSQL and serves the frontend from the
same Axum server binary.

## What It Does Today

- Accepts scan requests from the web UI or API.
- Validates submitted URLs and blocks unsafe/private targets in normal runtime.
- Runs accessibility analysis through Chromium + axe-core.
- Runs content-safety classification through Anthropic.
- Persists scans and findings in PostgreSQL.
- Returns scan status, phase, scores, risk level, findings, category breakdown,
  and recommended actions.
- Shows a complete frontend flow:
  - URL entry and empty state
  - loading states driven by scan phase
  - failure state with mapped operator-friendly messages
  - dashboard with summary, score cards, findings, breakdown, and re-scan
- Includes an end-to-end Playwright test that boots a fixture site, test
  Postgres, the server, and the SPA together.

## Main User-Facing Surfaces

- API:
  - `GET /api/healthz`
  - `POST /api/scans`
  - `GET /api/scans/:id`
- Web app:
  - single-page dashboard served by the Axum binary

## Architecture

- `crates/server`: Axum HTTP server, SPA hosting, API DTO shaping, startup
  config
- `crates/storage`: sqlx/PostgreSQL access and repository layer
- `crates/core`: shared domain enums, models, validation, scoring, aggregation
- `crates/browser`: Chromium wrapper, axe injection/parsing, accessibility
  mapping
- `crates/ai`: Anthropic client plus content-safety response parsing/mapping
- `crates/worker`: async scan pipeline and phase transitions
- `web/`: React + Vite + Tailwind + shadcn/ui frontend with TanStack Query

## Conventions And Decisions

- PostgreSQL is the only persistent store.
- The frontend must be built before the Rust binary because the server serves
  `web/dist`.
- Scans are asynchronous and phase-based: queued, loading, accessibility,
  content_safety, aggregating, completed, failed.
- Cached completed scans can be reused unless a re-scan is forced.
- The runtime product path is a single deployable backend process serving both
  API and SPA.
