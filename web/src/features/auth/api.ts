import { apiClient } from '@/lib/api-client'
import { apiCall } from '@/lib/api-helpers'
import type {
  AuthUser,
  LoginResponse,
  AuthCallbackResponse,
  RefreshTokenResponse,
} from '@zako-ac/zako3-data'

export const authApi = {
  getLoginUrl: async (): Promise<LoginResponse> => {
    return apiCall(apiClient.get<LoginResponse>('/auth/login'))
  },

  handleCallback: async (code: string): Promise<AuthCallbackResponse> => {
    return apiCall(
      apiClient.get<AuthCallbackResponse>(`/auth/callback?code=${code}`)
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
