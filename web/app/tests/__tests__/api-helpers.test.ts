import { describe, it, expect } from 'vitest'
import { unwrapApiResponse, apiCall } from '../../src/lib/api-helpers'
import type { ApiResponse } from '../../src/types/api'

describe('api-helpers', () => {
  describe('unwrapApiResponse', () => {
    it('should return data when no error', () => {
      const response: ApiResponse<{ id: string; name: string }> = {
        data: { id: '1', name: 'Test' },
      }

      const result = unwrapApiResponse(response)

      expect(result).toEqual({ id: '1', name: 'Test' })
    })

    it('should throw error when response contains error', () => {
      const response: ApiResponse<unknown> = {
        data: null as any,
        error: {
          code: 'TEST_ERROR',
          message: 'This is a test error',
        },
      }

      expect(() => unwrapApiResponse(response)).toThrow('This is a test error')
    })

    it('should handle various data types', () => {
      const stringResponse: ApiResponse<string> = { data: 'test' }
      expect(unwrapApiResponse(stringResponse)).toBe('test')

      const numberResponse: ApiResponse<number> = { data: 42 }
      expect(unwrapApiResponse(numberResponse)).toBe(42)

      const arrayResponse: ApiResponse<string[]> = { data: ['a', 'b', 'c'] }
      expect(unwrapApiResponse(arrayResponse)).toEqual(['a', 'b', 'c'])
    })
  })

  describe('apiCall', () => {
    it('should unwrap successful response', async () => {
      const response: ApiResponse<{ id: string }> = {
        data: { id: '1' },
      }

      const result = await apiCall(Promise.resolve(response))

      expect(result).toEqual({ id: '1' })
    })

    it('should throw error from failed response', async () => {
      const response: ApiResponse<unknown> = {
        data: null as any,
        error: {
          code: 'API_ERROR',
          message: 'API request failed',
        },
      }

      await expect(apiCall(Promise.resolve(response))).rejects.toThrow(
        'API request failed'
      )
    })

    it('should handle promise rejection', async () => {
      const rejectedPromise = Promise.reject(new Error('Network error'))

      await expect(apiCall(rejectedPromise)).rejects.toThrow('Network error')
    })
  })
})
