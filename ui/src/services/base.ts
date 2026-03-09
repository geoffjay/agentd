/**
 * ApiClient – base class for all agentd service clients.
 *
 * Features:
 * - Configurable base URL
 * - Automatic JSON serialization/deserialization
 * - Typed error responses (ApiError)
 * - Request timeout via AbortController
 * - Exponential-backoff retry for transient network errors
 * - Interceptor hooks for auth headers (future use)
 */

import { ApiError } from '@/types/common'

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

export interface ApiClientOptions {
  /** Base URL for all requests (no trailing slash) */
  baseUrl: string
  /** Request timeout in milliseconds (default 10 000) */
  timeoutMs?: number
  /** Max retry attempts for transient errors (default 3) */
  maxRetries?: number
  /** Initial backoff delay in milliseconds (default 200) */
  initialBackoffMs?: number
  /** Optional hook called before every request — useful for injecting auth headers */
  onRequest?: (init: RequestInit) => RequestInit | Promise<RequestInit>
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const DEFAULT_TIMEOUT_MS = 10_000
const DEFAULT_MAX_RETRIES = 3
const DEFAULT_INITIAL_BACKOFF_MS = 200

/** Returns true if the error is worth retrying */
function isTransient(err: unknown): boolean {
  // Network errors (TypeError from fetch) and 5xx responses are transient
  if (err instanceof TypeError) return true
  if (err instanceof ApiError && err.status >= 500) return true
  return false
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

// ---------------------------------------------------------------------------
// ApiClient
// ---------------------------------------------------------------------------

export class ApiClient {
  protected readonly baseUrl: string
  private readonly timeoutMs: number
  private readonly maxRetries: number
  private readonly initialBackoffMs: number
  private readonly onRequest?: ApiClientOptions['onRequest']

  constructor(options: ApiClientOptions) {
    this.baseUrl = options.baseUrl.replace(/\/$/, '')
    this.timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS
    this.maxRetries = options.maxRetries ?? DEFAULT_MAX_RETRIES
    this.initialBackoffMs = options.initialBackoffMs ?? DEFAULT_INITIAL_BACKOFF_MS
    this.onRequest = options.onRequest
  }

  // -------------------------------------------------------------------------
  // Core request method (with retry)
  // -------------------------------------------------------------------------

  protected async request<T>(
    method: string,
    path: string,
    body?: unknown,
    queryParams?: Record<string, string | number | boolean | undefined>,
  ): Promise<T> {
    const url = this.buildUrl(path, queryParams)
    let attempt = 0

    while (true) {
      try {
        return await this.executeRequest<T>(method, url, body)
      } catch (err) {
        attempt++
        if (attempt >= this.maxRetries || !isTransient(err)) {
          throw err
        }
        const backoff = this.initialBackoffMs * 2 ** (attempt - 1)
        await sleep(backoff)
      }
    }
  }

  private async executeRequest<T>(method: string, url: string, body?: unknown): Promise<T> {
    const controller = new AbortController()
    const timerId = setTimeout(() => controller.abort(), this.timeoutMs)

    let init: RequestInit = {
      method,
      headers: {
        'Content-Type': 'application/json',
        Accept: 'application/json',
      },
      signal: controller.signal,
    }

    if (body !== undefined) {
      init = { ...init, body: JSON.stringify(body) }
    }

    if (this.onRequest) {
      init = await this.onRequest(init)
    }

    try {
      const response = await fetch(url, init)
      clearTimeout(timerId)
      return await this.parseResponse<T>(response)
    } catch (err) {
      clearTimeout(timerId)
      if (err instanceof ApiError) throw err
      // AbortController fires a DOMException with name 'AbortError'
      if (err instanceof DOMException && err.name === 'AbortError') {
        throw new ApiError(408, `Request timed out after ${this.timeoutMs}ms`)
      }
      throw new ApiError(0, err instanceof Error ? err.message : String(err))
    }
  }

  // -------------------------------------------------------------------------
  // Response parsing
  // -------------------------------------------------------------------------

  private async parseResponse<T>(response: Response): Promise<T> {
    const contentType = response.headers.get('content-type') ?? ''

    if (response.status === 204 || response.headers.get('content-length') === '0') {
      return undefined as T
    }

    if (!response.ok) {
      let message = `HTTP ${response.status}`
      let errorBody: unknown

      if (contentType.includes('application/json')) {
        try {
          errorBody = await response.json()
          if (
            errorBody &&
            typeof errorBody === 'object' &&
            'error' in errorBody &&
            typeof (errorBody as Record<string, unknown>).error === 'string'
          ) {
            message = (errorBody as { error: string }).error
          }
        } catch {
          // fall through with default message
        }
      } else {
        message = (await response.text().catch(() => message)) || message
      }

      throw new ApiError(response.status, message, errorBody)
    }

    if (contentType.includes('application/json')) {
      return response.json() as Promise<T>
    }

    // Fallback: return raw text cast to T
    return response.text() as unknown as T
  }

  // -------------------------------------------------------------------------
  // URL construction
  // -------------------------------------------------------------------------

  private buildUrl(
    path: string,
    params?: Record<string, string | number | boolean | undefined>,
  ): string {
    const url = new URL(`${this.baseUrl}${path}`)

    if (params) {
      for (const [key, value] of Object.entries(params)) {
        if (value !== undefined) {
          url.searchParams.set(key, String(value))
        }
      }
    }

    return url.toString()
  }

  // -------------------------------------------------------------------------
  // Convenience wrappers
  // -------------------------------------------------------------------------

  protected get<T>(
    path: string,
    params?: Record<string, string | number | boolean | undefined>,
  ): Promise<T> {
    return this.request<T>('GET', path, undefined, params)
  }

  protected post<T>(path: string, body?: unknown): Promise<T> {
    return this.request<T>('POST', path, body)
  }

  protected put<T>(path: string, body?: unknown): Promise<T> {
    return this.request<T>('PUT', path, body)
  }

  protected delete<T>(path: string): Promise<T> {
    return this.request<T>('DELETE', path)
  }

  // -------------------------------------------------------------------------
  // WebSocket helper
  // -------------------------------------------------------------------------

  /**
   * Opens a WebSocket connection, converting the base URL scheme if needed:
   * http → ws, https → wss
   */
  protected openWebSocket(path: string): WebSocket {
    const wsBase = this.baseUrl.replace(/^http/, 'ws')
    return new WebSocket(`${wsBase}${path}`)
  }
}
