import { apiClient, buildQueryString } from '@/lib/api-client'
import { apiCall } from '@/lib/api-helpers'
import type {
  PaginatedResponse,
  PaginationParams,
  Tap,
  TapWithAccess,
  TapFilters,
  TapSort,
  TapStats,
  TapAuditLogEntry,
  TapApiToken,
  TapApiTokenCreated,
} from '@zako-ac/zako3-data'
import type {
  CreateTapInput,
  UpdateTapInput,
  ReportTapInput,
  VerificationRequestInput,
  CreateTapApiTokenInput,
  UpdateTapApiTokenInput,
} from '@zako-ac/zako3-data'

interface GetTapsParams extends Partial<PaginationParams>, Partial<TapFilters> {
  sortField?: TapSort['field']
  sortDirection?: TapSort['direction']
}

export const tapsApi = {
  getTaps: async (
    params: GetTapsParams = {}
  ): Promise<PaginatedResponse<TapWithAccess>> => {
    const query = buildQueryString({
      page: params.page,
      perPage: params.perPage,
      search: params.search,
      roles: params.roles?.join(','),
      accessible: params.accessible,
      ownerId: params.ownerId,
      sortField: params.sortField,
      sortDirection: params.sortDirection,
    })
    return apiCall(
      apiClient.get<PaginatedResponse<TapWithAccess>>(`/taps${query}`)
    )
  },

  getTap: async (tapId: string): Promise<TapWithAccess> => {
    return apiCall(apiClient.get<TapWithAccess>(`/taps/${tapId}`))
  },

  createTap: async (data: CreateTapInput): Promise<Tap> => {
    return apiCall(apiClient.post<Tap>('/taps', data))
  },

  updateTap: async (tapId: string, data: UpdateTapInput): Promise<Tap> => {
    return apiCall(apiClient.patch<Tap>(`/taps/${tapId}`, data))
  },

  deleteTap: async (tapId: string): Promise<void> => {
    return apiCall(apiClient.delete(`/taps/${tapId}`))
  },

  getTapStats: async (tapId: string): Promise<TapStats> => {
    return apiCall(apiClient.get<TapStats>(`/taps/${tapId}/stats`))
  },

  getTapAuditLog: async (
    tapId: string,
    params: Partial<PaginationParams> = {}
  ): Promise<PaginatedResponse<TapAuditLogEntry>> => {
    const query = buildQueryString({
      page: params.page,
      perPage: params.perPage,
    })
    return apiCall(
      apiClient.get<PaginatedResponse<TapAuditLogEntry>>(
        `/taps/${tapId}/audit-log${query}`
      )
    )
  },

  reportTap: async (tapId: string, data: ReportTapInput): Promise<void> => {
    return apiCall(apiClient.post(`/taps/${tapId}/report`, data))
  },

  requestVerification: async (
    tapId: string,
    data: VerificationRequestInput
  ): Promise<void> => {
    return apiCall(apiClient.post(`/taps/${tapId}/verify`, data))
  },

  getMyTaps: async (
    params: Partial<PaginationParams> = {}
  ): Promise<PaginatedResponse<Tap>> => {
    const query = buildQueryString({
      page: params.page,
      perPage: params.perPage,
    })
    return apiCall(
      apiClient.get<PaginatedResponse<Tap>>(`/users/me/taps${query}`)
    )
  },

  // API Token management
  getTapApiTokens: async (tapId: string): Promise<TapApiToken[]> => {
    return apiCall(apiClient.get<TapApiToken[]>(`/taps/${tapId}/api-tokens`))
  },

  createTapApiToken: async (
    tapId: string,
    data: CreateTapApiTokenInput
  ): Promise<TapApiTokenCreated> => {
    return apiCall(
      apiClient.post<TapApiTokenCreated>(`/taps/${tapId}/api-tokens`, data)
    )
  },

  updateTapApiToken: async (
    tapId: string,
    tokenId: string,
    data: UpdateTapApiTokenInput
  ): Promise<TapApiToken> => {
    return apiCall(
      apiClient.patch<TapApiToken>(`/taps/${tapId}/api-tokens/${tokenId}`, data)
    )
  },

  regenerateTapApiToken: async (
    tapId: string,
    tokenId: string
  ): Promise<TapApiTokenCreated> => {
    return apiCall(
      apiClient.post<TapApiTokenCreated>(
        `/taps/${tapId}/api-tokens/${tokenId}/regenerate`
      )
    )
  },

  deleteTapApiToken: async (tapId: string, tokenId: string): Promise<void> => {
    return apiCall(apiClient.delete(`/taps/${tapId}/api-tokens/${tokenId}`))
  },
}
