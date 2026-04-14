import { apiClient, buildQueryString } from '@/lib/api-client'
import { apiCall } from '@/lib/api-helpers'
import type {
  PaginatedResponse,
  PaginationParams,
  AdminActivity,
  Tap,
  VerificationRequestFull,
  VerificationStatus,
  UpdateTapInput,
  TapOccupation,
} from '@zako-ac/zako3-data'

interface GetVerificationRequestsParams extends Partial<PaginationParams> {
  status?: VerificationStatus
}

export const adminApi = {
  updateTap: async (tapId: string, data: UpdateTapInput): Promise<Tap> => {
    return apiCall(apiClient.patch<Tap>(`/admin/taps/${tapId}`, data))
  },

  updateTapOccupation: async (
    tapId: string,
    occupation: TapOccupation
  ): Promise<Tap> => {
    return apiCall(
      apiClient.patch<Tap>(`/admin/taps/${tapId}/occupation`, { occupation })
    )
  },

  getActivity: async (
    params: Partial<PaginationParams> = {}
  ): Promise<PaginatedResponse<AdminActivity>> => {
    const query = buildQueryString({
      page: params.page,
      perPage: params.perPage,
    })
    return apiCall(
      apiClient.get<PaginatedResponse<AdminActivity>>(`/admin/activity${query}`)
    )
  },

  getPendingVerifications: async (): Promise<Tap[]> => {
    return apiCall(apiClient.get<Tap[]>('/admin/taps/pending-verification'))
  },

  getVerificationRequests: async (
    params: GetVerificationRequestsParams = {}
  ): Promise<PaginatedResponse<VerificationRequestFull>> => {
    const query = buildQueryString({
      page: params.page,
      perPage: params.perPage,
      status: params.status,
    })
    return apiCall(
      apiClient.get<PaginatedResponse<VerificationRequestFull>>(`/admin/verifications${query}`)
    )
  },

  approveVerification: async (requestId: string): Promise<void> => {
    return apiCall(apiClient.post(`/admin/verifications/${requestId}/approve`))
  },

  rejectVerification: async (
    requestId: string,
    reason: string
  ): Promise<void> => {
    return apiCall(
      apiClient.post(`/admin/verifications/${requestId}/reject`, { reason })
    )
  },

  getStats: async (): Promise<{ globalUniqueUsers: number }> => {
    return apiCall(apiClient.get<{ globalUniqueUsers: number }>('/admin/stats'))
  },
}
