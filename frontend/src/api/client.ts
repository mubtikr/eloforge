export class ApiError extends Error {
  readonly status: number
  readonly code: string

  constructor(status: number, code: string, message: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.code = code
  }
}

type Json = Record<string, unknown>

export async function api<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers)
  if (init.body !== undefined && !headers.has('content-type')) {
    headers.set('content-type', 'application/json')
  }

  const response = await fetch(path, {
    ...init,
    headers,
    credentials: 'include',
  })

  if (!response.ok) {
    let code = 'http_error'
    let message = `Request failed (${response.status})`
    try {
      const body = (await response.json()) as Json
      const error = body.error
      if (error && typeof error === 'object') {
        const detail = error as Json
        if (typeof detail.code === 'string') code = detail.code
        if (typeof detail.message === 'string') message = detail.message
      }
    } catch {
      // Non-JSON error body; keep the status-based message.
    }
    throw new ApiError(response.status, code, message)
  }

  if (response.status === 204) {
    return undefined as T
  }
  return (await response.json()) as T
}

export function getErrorMessage(error: unknown): string {
  if (error instanceof ApiError) return error.message
  if (error instanceof Error) return error.message
  return 'Something went wrong. Please try again.'
}
