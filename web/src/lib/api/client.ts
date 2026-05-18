import type {
  CreateScanRequest,
  CreateScanResponse,
  ErrorResponse,
  ScanResponse,
} from "@/lib/api/types"

const DEFAULT_API_BASE_PATH = "/api"

export class ApiClientError extends Error {
  readonly status: number

  constructor(message: string, status: number) {
    super(message)
    this.name = "ApiClientError"
    this.status = status
  }
}

export type ApiClientOptions = {
  baseUrl?: string
  fetchImpl?: typeof fetch
}

export class ApiClient {
  private readonly baseUrl: string
  private readonly fetchImpl: typeof fetch

  constructor(options: ApiClientOptions = {}) {
    this.baseUrl = sanitizeBaseUrl(options.baseUrl ?? DEFAULT_API_BASE_PATH)
    this.fetchImpl = options.fetchImpl ?? fetch
  }

  async createScan(request: CreateScanRequest): Promise<CreateScanResponse> {
    return this.request<CreateScanResponse>("/scans", {
      method: "POST",
      body: JSON.stringify(request),
    })
  }

  async getScan(scanId: number): Promise<ScanResponse> {
    return this.request<ScanResponse>(`/scans/${scanId}`)
  }

  private async request<T>(path: string, init: RequestInit = {}): Promise<T> {
    const response = await this.fetchImpl(`${this.baseUrl}${path}`, {
      ...init,
      headers: {
        "Content-Type": "application/json",
        ...init.headers,
      },
    })

    if (!response.ok) {
      throw await buildApiClientError(response)
    }

    return (await response.json()) as T
  }
}

async function buildApiClientError(response: Response) {
  const fallbackMessage = `Request failed with status ${response.status}.`

  try {
    const body = (await response.json()) as Partial<ErrorResponse>
    const message =
      typeof body.error === "string" && body.error.length > 0
        ? body.error
        : fallbackMessage

    return new ApiClientError(message, response.status)
  } catch {
    return new ApiClientError(fallbackMessage, response.status)
  }
}

function sanitizeBaseUrl(baseUrl: string) {
  return baseUrl.endsWith("/") ? baseUrl.slice(0, -1) : baseUrl
}

export const apiClient = new ApiClient({
  baseUrl: import.meta.env.VITE_API_BASE_URL ?? DEFAULT_API_BASE_PATH,
})
