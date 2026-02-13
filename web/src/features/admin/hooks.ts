import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { adminApi } from './api'
import type { PaginationParams, VerificationStatus } from '@zako-ac/zako3-data'

interface GetVerificationRequestsParams extends Partial<PaginationParams> {
  status?: VerificationStatus
}

export const adminKeys = {
  all: ['admin'] as const,
  activity: (params: Partial<PaginationParams>) =>
    [...adminKeys.all, 'activity', params] as const,
  pendingVerifications: () =>
    [...adminKeys.all, 'pending-verifications'] as const,
  verifications: (params: GetVerificationRequestsParams) =>
    [...adminKeys.all, 'verifications', params] as const,
}

export const useAdminActivity = (params: Partial<PaginationParams> = {}) => {
  return useQuery({
    queryKey: adminKeys.activity(params),
    queryFn: () => adminApi.getActivity(params),
  })
}

export const usePendingVerifications = () => {
  return useQuery({
    queryKey: adminKeys.pendingVerifications(),
    queryFn: () => adminApi.getPendingVerifications(),
  })
}

export const useVerificationRequests = (
  params: GetVerificationRequestsParams = {}
) => {
  return useQuery({
    queryKey: adminKeys.verifications(params),
    queryFn: () => adminApi.getVerificationRequests(params),
  })
}

export const useApproveVerification = () => {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (requestId: string) => adminApi.approveVerification(requestId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: adminKeys.all })
    },
  })
}

export const useRejectVerification = () => {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: ({
      requestId,
      reason,
    }: {
      requestId: string
      reason: string
    }) => adminApi.rejectVerification(requestId, reason),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: adminKeys.all })
    },
  })
}
