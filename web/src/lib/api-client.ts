import type { ApiError, ApiResponse } from '@zako-ac/zako3-data'
import { API_BASE_URL, AUTH_TOKEN_KEY } from './constants'

type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE'

interface RequestOptions {
  method?: HttpMethod
  body?: unknown
  headers?: Record<string, string>
  signal?: AbortSignal
}

class ApiClient {
  private baseUrl: string

  constructor(baseUrl: string) {
    this.baseUrl = baseUrl
  }

  private getToken(): string | null {
    return localStorage.getItem(AUTH_TOKEN_KEY)
  }

  private async request<T>(
    endpoint: string,
    options: RequestOptions = {}
  ): Promise<ApiResponse<T>> {
    const { method = 'GET', body, headers = {}, signal } = options

    const token = this.getToken()
    const requestHeaders: Record<string, string> = {
      'Content-Type': 'application/json',
      ...headers,
    }

    if (token) {
      requestHeaders['Authorization'] = `Bearer ${token}`
    }

    const url = `${this.baseUrl}${endpoint}`
    const config: RequestInit = {
      method,
      headers: requestHeaders,
      signal,
    }

    if (body && method !== 'GET') {
      config.body = JSON.stringify(body)
    }

    const response = await fetch(url, config)

    if (!response.ok) {
      const error: ApiError = await response.json().catch(() => ({
        code: 'UNKNOWN_ERROR',
        message: response.statusText,
      }))

      if (response.status === 401) {
        localStorage.removeItem(AUTH_TOKEN_KEY)
        localStorage.removeItem('zako_auth_user') // Clear Zustand persist store
        window.location.href = '/login'
      }

      return { data: null as T, error }
    }

    if (response.status === 204) {
      return { data: null as T }
    }

    const data = await response.json()
    return { data }
  }

  get<T>(endpoint: string, signal?: AbortSignal): Promise<ApiResponse<T>> {
    return this.request<T>(endpoint, { method: 'GET', signal })
  }

  post<T>(
    endpoint: string,
    body?: unknown,
    signal?: AbortSignal
  ): Promise<ApiResponse<T>> {
    return this.request<T>(endpoint, { method: 'POST', body, signal })
  }

  put<T>(
    endpoint: string,
    body?: unknown,
    signal?: AbortSignal
  ): Promise<ApiResponse<T>> {
    return this.request<T>(endpoint, { method: 'PUT', body, signal })
  }

  patch<T>(
    endpoint: string,
    body?: unknown,
    signal?: AbortSignal
  ): Promise<ApiResponse<T>> {
    return this.request<T>(endpoint, { method: 'PATCH', body, signal })
  }

  delete<T>(endpoint: string, signal?: AbortSignal): Promise<ApiResponse<T>> {
    return this.request<T>(endpoint, { method: 'DELETE', signal })
  }

  async postFormData<T>(endpoint: string, body: FormData): Promise<ApiResponse<T>> {
    return this.requestFormData<T>(endpoint, 'POST', body)
  }

  async putFormData<T>(endpoint: string, body: FormData): Promise<ApiResponse<T>> {
    return this.requestFormData<T>(endpoint, 'PUT', body)
  }

  private async requestFormData<T>(
    endpoint: string,
    method: 'POST' | 'PUT',
    body: FormData
  ): Promise<ApiResponse<T>> {
    const token = this.getToken()
    const headers: Record<string, string> = {}
    if (token) {
      headers['Authorization'] = `Bearer ${token}`
    }
    // Do NOT set Content-Type — browser sets it with the multipart boundary

    const url = `${this.baseUrl}${endpoint}`
    const response = await fetch(url, { method, headers, body })

    if (!response.ok) {
      const error: ApiError = await response.json().catch(() => ({
        code: 'UNKNOWN_ERROR',
        message: response.statusText,
      }))

      if (response.status === 401) {
        localStorage.removeItem(AUTH_TOKEN_KEY)
        localStorage.removeItem('zako_auth_user')
        window.location.href = '/login'
      }

      return { data: null as T, error }
    }

    if (response.status === 204) {
      return { data: null as T }
    }

    const data = await response.json()
    return { data }
  }
}

export const apiClient = new ApiClient(API_BASE_URL)

export const buildQueryString = (
  params: Record<string, string | number | boolean | undefined | null>
): string => {
  const searchParams = new URLSearchParams()

  Object.entries(params).forEach(([key, value]) => {
    if (value !== undefined && value !== null && value !== '') {
      searchParams.append(key, String(value))
    }
  })

  const query = searchParams.toString()
  return query ? `?${query}` : ''
}
