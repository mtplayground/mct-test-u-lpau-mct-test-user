# mct-test-u-lpau-mct-test-user

ZeroClaw is a Rust + React website scanner. The Axum server serves the compiled
SPA from `web/dist`, stores scan state in PostgreSQL, runs accessibility checks
through Chromium + axe-core, and runs content-safety classification through the
Anthropic Messages API.

## Prerequisites

- Rust toolchain with Cargo
- Node.js and npm
- PostgreSQL 16+ reachable through `DATABASE_URL`
- A Chromium-compatible browser binary installed on the host
- An Anthropic API key with access to the Messages API

Typical Linux Chromium paths:

- `/usr/bin/chromium`
- `/usr/bin/chromium-browser`
- `/usr/bin/google-chrome`

## Environment Variables

The server fails fast on missing required environment variables.

Required:

- `DATABASE_URL`: PostgreSQL connection string
- `ANTHROPIC_API_KEY`: API key used for content-safety classification
- `CHROMIUM_PATH`: absolute path to the Chromium binary
- `SCAN_TIMEOUT_SECS`: timeout budget for page scanning work
- `PORT`: HTTP port bound by the Axum server on `0.0.0.0:$PORT`

Example local config:

```bash
cp .env.example .env
```

`.env.example` contains:

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/zeroclaw
ANTHROPIC_API_KEY=sk-ant-example
CHROMIUM_PATH=/usr/bin/chromium
SCAN_TIMEOUT_SECS=60
PORT=8080
```

## Build Order

The frontend must be built before the Rust binary, because the Axum server
serves the compiled SPA from `web/dist`.

```bash
cd /workspace/web
npm install
npm run build

cd /workspace
PATH=/usr/local/cargo/bin:$PATH cargo build --release
```

Resulting server binary:

```bash
./target/release/zeroclaw-server
```

## Database Migrations

Run migrations before first startup and before deploying schema changes.

Using the helper script:

```bash
cd /workspace
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/zeroclaw
./scripts/sqlx-migrate.sh
```

Or directly with `sqlx`:

```bash
cd /workspace
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/zeroclaw
sqlx migrate run
```

## Local Startup

```bash
cd /workspace
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/zeroclaw
export ANTHROPIC_API_KEY=sk-ant-example
export CHROMIUM_PATH=/usr/bin/chromium
export SCAN_TIMEOUT_SECS=60
export PORT=8080

./scripts/sqlx-migrate.sh
PATH=/usr/local/cargo/bin:$PATH cargo build --release
./target/release/zeroclaw-server
```

The server listens on `0.0.0.0:$PORT`.

Routes:

- `GET /api/healthz`
- `POST /api/scans`
- `GET /api/scans/:id`

Non-API routes fall back to the SPA entrypoint.

## Deployment Notes

- Build and ship both the Rust server binary and the `web/dist` assets.
- Keep `web/dist` adjacent to the server working directory, or preserve the same
  project layout used in this repository.
- Run migrations during deploy before sending traffic to the new server.
- Ensure the host can launch the Chromium binary configured in
  `CHROMIUM_PATH`.
- Use PostgreSQL for all persistent state. Do not replace it with SQLite,
  JSON-file storage, or in-memory storage.
- If your PostgreSQL provider exposes both pooled and direct URLs, prefer the
  direct URL variant if you later need session-level Postgres features.

## Sample Reverse Proxy

Example Nginx configuration forwarding traffic to the Axum server on
`127.0.0.1:8080`:

```nginx
server {
    listen 80;
    server_name scanner.example.com;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

This works for both the SPA and `/api/*` routes because the Axum process serves
both behind the same listener.
