import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { adminApi } from './api'
import type { PaginationParams, VerificationStatus, TapOccupation } from '@zako-ac/zako3-data'
import { tapKeys } from '../taps/hooks'

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
  stats: () => [...adminKeys.all, 'stats'] as const,
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
    refetchInterval: 60_000,
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

export const useUpdateTapOccupation = (tapId: string) => {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (occupation: TapOccupation) =>
      adminApi.updateTapOccupation(tapId, occupation),
    onSuccess: (updatedTap: any) => {
      queryClient.setQueryData(tapKeys.detail(tapId), (old: any) => {
        if (!old) return updatedTap
        return { ...old, ...updatedTap }
      })
      queryClient.invalidateQueries({ queryKey: tapKeys.lists() })
    },
  })
}

export const useAdminStats = () => {
  return useQuery({
    queryKey: adminKeys.stats(),
    queryFn: () => adminApi.getStats(),
  })
}
