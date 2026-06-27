import { apiClient } from '@/lib/api-client'
import { apiCall } from '@/lib/api-helpers'
import type {
  UserApiKey,
  UserApiKeyCreated,
  CreateUserApiKeyInput,
  UpdateUserApiKeyInput,
} from '@zako-ac/zako3-data'

export const apiKeysApi = {
  list: async (): Promise<UserApiKey[]> => {
    return apiCall(apiClient.get<UserApiKey[]>('/users/me/api-keys'))
  },

  create: async (data: CreateUserApiKeyInput): Promise<UserApiKeyCreated> => {
    return apiCall(apiClient.post<UserApiKeyCreated>('/users/me/api-keys', data))
  },

  update: async (
    keyId: string,
    data: UpdateUserApiKeyInput
  ): Promise<UserApiKey> => {
    return apiCall(apiClient.patch<UserApiKey>(`/users/me/api-keys/${keyId}`, data))
  },

  revoke: async (keyId: string): Promise<void> => {
    return apiCall(apiClient.delete(`/users/me/api-keys/${keyId}`))
  },
}
