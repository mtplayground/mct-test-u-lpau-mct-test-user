import { test, expect } from "@playwright/test"
import { createServer, type Server } from "node:http"
import { mkdtempSync, readFileSync, rmSync, writeFileSync, existsSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { execFileSync, spawn, type ChildProcess } from "node:child_process"
import net from "node:net"

const WORKSPACE_ROOT = "/workspace"
const WEB_ROOT = path.join(WORKSPACE_ROOT, "web")
const POSTGRES_BIN_DIR = "/usr/lib/postgresql/16/bin"
const FIXTURE_HTML = readFileSync(
  path.join(WEB_ROOT, "tests/e2e/fixtures/site/index.html"),
  "utf8",
)

type TestStack = {
  appProcess: ChildProcess
  appUrl: string
  databaseName: string
  dataDir: string
  fixtureServer: Server
  fixtureUrl: string
  logFile: string
  port: number
  socketDir: string
}

let stack: TestStack | null = null

test.beforeAll(async () => {
  ensureBuildArtifacts()
  stack = await startTestStack()
})

test.afterAll(async () => {
  if (stack !== null) {
    await stopTestStack(stack)
    stack = null
  }
})

test("scans the fixture website end to end and renders the dashboard", async ({
  page,
}) => {
  const runningStack = stack

  if (runningStack === null) {
    throw new Error("test stack was not initialized")
  }

  await page.goto(runningStack.appUrl)
  await page.getByLabel("Website URL").fill(runningStack.fixtureUrl)
  const createScanResponsePromise = page.waitForResponse((response) => {
    return (
      response.url().includes("/api/scans") &&
      response.request().method() === "POST"
    )
  })
  await page.getByRole("button", { name: "Analyze Website" }).click()
  const createScanResponse = await createScanResponsePromise
  expect(createScanResponse.status()).toBe(200)

  await expect(page.getByTestId("dashboard")).toBeVisible({ timeout: 30_000 })

  await expect(page.getByTestId("accessibility-score-card-value")).toHaveText("1")
  await expect(page.getByTestId("inappropriate-score-card-value")).toHaveText("8")
  await expect(page.getByTestId("risk-level-badge")).toContainText("high")
  await expect(page.getByTestId("accessibility-findings")).toContainText(
    "Images must have alternative text",
  )
  await expect(page.getByTestId("inappropriate-findings")).toContainText(
    "Weapon promotion",
  )
})

function ensureBuildArtifacts() {
  execFileSync("npm", ["run", "build"], {
    cwd: WEB_ROOT,
    stdio: "inherit",
  })
  execFileSync(
    "bash",
    ["-lc", "PATH=/usr/local/cargo/bin:$PATH cargo build -p zeroclaw-server --tests"],
    {
      cwd: WORKSPACE_ROOT,
      stdio: "inherit",
    },
  )
}

async function startTestStack(): Promise<TestStack> {
  const fixturePort = await getFreePort()
  const appPort = await getFreePort()
  const postgresPort = await getFreePort()
  const fixtureUrl = `http://127.0.0.1:${fixturePort}/`
  const fixtureServer = await startFixtureServer(fixturePort)

  const dataDir = createTempPath("zeroclaw-playwright-pgdata-")
  const socketDir = createTempPath("zeroclaw-playwright-pgsock-")
  const logFile = path.join(tmpdir(), `zeroclaw-playwright-pglog-${Date.now()}`)
  writeFileSync(logFile, "")
  chownToPostgres(logFile)

  runAsPostgres(["initdb", "-A", "trust", "-U", "postgres", "-D", dataDir])
  runAsPostgres([
    "pg_ctl",
    "-D",
    dataDir,
    "-l",
    logFile,
    "-o",
    `-h 127.0.0.1 -k ${socketDir} -p ${postgresPort}`,
    "start",
  ])

  const databaseName = `playwright_${Date.now()}`
  runAsPostgres([
    "createdb",
    "-h",
    "127.0.0.1",
    "-p",
    `${postgresPort}`,
    "-U",
    "postgres",
    databaseName,
  ])

  const appProcess = spawn(path.join(WORKSPACE_ROOT, "target/debug/zeroclaw-server"), [], {
    cwd: WORKSPACE_ROOT,
    env: {
      ...process.env,
      DATABASE_URL: `postgresql://postgres@127.0.0.1:${postgresPort}/${databaseName}`,
      ANTHROPIC_API_KEY: "playwright-fixture-key",
      CHROMIUM_PATH: "/usr/bin/chromium",
      PORT: `${appPort}`,
      SCAN_TIMEOUT_SECS: "30",
      ZEROCLAW_ALLOW_PRIVATE_URLS: "true",
      ZEROCLAW_E2E_FIXTURE_URL: fixtureUrl,
    },
    stdio: "inherit",
  })

  await waitForHttp(`http://127.0.0.1:${appPort}/api/healthz`)

  return {
    appProcess,
    appUrl: `http://127.0.0.1:${appPort}`,
    databaseName,
    dataDir,
    fixtureServer,
    fixtureUrl,
    logFile,
    port: postgresPort,
    socketDir,
  }
}

async function stopTestStack(stack: TestStack) {
  stack.appProcess.kill("SIGTERM")
  await onceClosed(stack.appProcess)

  await new Promise<void>((resolve, reject) => {
    stack.fixtureServer.close((error) => {
      if (error) {
        reject(error)
        return
      }

      resolve()
    })
  })

  try {
    runAsPostgres(["pg_ctl", "-D", stack.dataDir, "stop", "-m", "fast"])
  } catch {
    // ignore cleanup failures in teardown
  }

  if (existsSync(stack.dataDir)) {
    rmSync(stack.dataDir, { recursive: true, force: true })
  }
  if (existsSync(stack.socketDir)) {
    rmSync(stack.socketDir, { recursive: true, force: true })
  }
  if (existsSync(stack.logFile)) {
    rmSync(stack.logFile, { force: true })
  }
}

function runAsPostgres(args: string[]) {
  const [binaryName, ...rest] = args
  execFileSync(
    "runuser",
    ["-u", "postgres", "--", path.join(POSTGRES_BIN_DIR, binaryName), ...rest],
    { stdio: "inherit" },
  )
}

function createTempPath(prefix: string) {
  const directory = mkdtempSync(path.join(tmpdir(), prefix))
  chownToPostgres(directory)
  return directory
}

function chownToPostgres(targetPath: string) {
  execFileSync("chown", ["postgres:postgres", targetPath], { stdio: "inherit" })
}

function startFixtureServer(port: number) {
  return new Promise<Server>((resolve, reject) => {
    const server = createServer((request, response) => {
      if (request.url === "/hero.png") {
        response.writeHead(200, { "Content-Type": "image/png" })
        response.end("")
        return
      }

      response.writeHead(200, { "Content-Type": "text/html; charset=utf-8" })
      response.end(FIXTURE_HTML)
    })

    server.on("error", reject)
    server.listen(port, "127.0.0.1", () => resolve(server))
  })
}

function getFreePort() {
  return new Promise<number>((resolve, reject) => {
    const server = net.createServer()
    server.on("error", reject)
    server.listen(0, "127.0.0.1", () => {
      const address = server.address()

      if (address === null || typeof address === "string") {
        reject(new Error("failed to allocate a TCP port"))
        return
      }

      const { port } = address
      server.close((error) => {
        if (error) {
          reject(error)
          return
        }

        resolve(port)
      })
    })
  })
}

async function waitForHttp(url: string) {
  for (let attempt = 0; attempt < 120; attempt += 1) {
    try {
      const response = await fetch(url)
      if (response.ok) {
        return
      }
    } catch {
      // keep polling until the service is ready
    }

    await new Promise((resolve) => setTimeout(resolve, 500))
  }

  throw new Error(`timed out waiting for ${url}`)
}

function onceClosed(process: ChildProcess) {
  return new Promise<void>((resolve) => {
    if (process.exitCode !== null) {
      resolve()
      return
    }

    process.once("exit", () => resolve())
  })
}
