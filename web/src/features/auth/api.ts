import { apiClient } from '@/lib/api-client'
import { apiCall } from '@/lib/api-helpers'
import type {
  AuthUser,
  AuthCallbackResponse,
  RefreshTokenResponse,
} from '@zako-ac/zako3-data'

export const authApi = {
  handleCallback: async (code: string, state: string | null): Promise<AuthCallbackResponse> => {
    const params = new URLSearchParams({ code })
    if (state) params.set('state', state)
    return apiCall(
      apiClient.get<AuthCallbackResponse>(`/auth/callback?${params}`)
    )
  },

  logout: async (): Promise<void> => {
    await apiClient.post('/auth/logout')
  },

  refreshToken: async (): Promise<RefreshTokenResponse> => {
    return apiCall(apiClient.get<RefreshTokenResponse>('/auth/refresh'))
  },

  getCurrentUser: async (): Promise<AuthUser> => {
    return apiCall(apiClient.get<AuthUser>('/users/me'))
  },
}
