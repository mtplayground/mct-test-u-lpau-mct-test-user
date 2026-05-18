# mct-test-u-lpau-mct-test-user

## Build Order

The Axum binary serves the compiled SPA from `web/dist`, so build the frontend
before starting the Rust server.

```bash
cd /workspace/web
npm install
npm run build

cd /workspace
PATH=/usr/local/cargo/bin:$PATH cargo build
PATH=/usr/local/cargo/bin:$PATH cargo run --bin zeroclaw-server
```

The API is mounted under `/api`, and the built frontend handles non-API routes
through the SPA fallback.
