import { defineConfig } from "@playwright/test"

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: false,
  retries: 0,
  timeout: 180_000,
  workers: 1,
  use: {
    baseURL: "http://127.0.0.1:4310",
    headless: true,
  },
})
