import type { ApiResponse } from '@zako-ac/zako3-data'

/**
 * Unwraps an API response and throws an error if the response contains one.
 * This eliminates the need for repetitive error checking across API calls.
 *
 * @template T - The expected data type
 * @param response - The API response to unwrap
 * @returns The unwrapped data
 * @throws {Error} When the response contains an error
 *
 * @example
 * ```typescript
 * const response = await apiClient.get<User>('/users/123')
 * const user = unwrapApiResponse(response) // throws if error, returns data otherwise
 * ```
 */
export function unwrapApiResponse<T>(response: ApiResponse<T>): T {
  if (response.error) {
    throw new Error(response.error.message)
  }
  return response.data
}

/**
 * Type-safe wrapper for API calls with automatic error handling.
 * Simplifies API method implementations by handling the response unwrapping.
 *
 * @template T - The expected data type
 * @param responsePromise - A promise that resolves to an API response
 * @returns A promise that resolves to the unwrapped data
 * @throws {Error} When the response contains an error
 *
 * @example
 * ```typescript
 * // Instead of:
 * const response = await apiClient.get<User>('/users/123')
 * if (response.error) throw new Error(response.error.message)
 * return response.data
 *
 * // Use:
 * return apiCall(apiClient.get<User>('/users/123'))
 * ```
 */
export async function apiCall<T>(
  responsePromise: Promise<ApiResponse<T>>
): Promise<T> {
  const response = await responsePromise
  return unwrapApiResponse(response)
}
