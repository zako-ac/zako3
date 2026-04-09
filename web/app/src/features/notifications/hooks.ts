import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { notificationsApi } from './api'
import type { NotificationFilters, NotificationSort, PaginationParams } from '@zako-ac/zako3-data'

export const notificationKeys = {
  all: ['notifications'] as const,
  lists: () => [...notificationKeys.all, 'list'] as const,
  list: (filters: UseNotificationsParams) => [...notificationKeys.lists(), filters] as const,
  unreadCount: () => [...notificationKeys.all, 'unread-count'] as const,
  adminLists: () => [...notificationKeys.all, 'admin-list'] as const,
  adminList: (filters: UseNotificationsParams) => [...notificationKeys.adminLists(), filters] as const,
}

interface UseNotificationsParams
  extends Partial<PaginationParams>,
    Partial<NotificationFilters> {
  sortField?: NotificationSort['field']
  sortDirection?: NotificationSort['direction']
}

export const useNotifications = (params: UseNotificationsParams = {}) => {
  return useQuery({
    queryKey: notificationKeys.list(params),
    queryFn: () => notificationsApi.getNotifications(params),
  })
}

export const useUnreadCount = () => {
  return useQuery({
    queryKey: notificationKeys.unreadCount(),
    queryFn: () => notificationsApi.getUnreadCount(),
    refetchInterval: 30000,
  })
}

export const useMarkAsRead = () => {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (notificationId: string) => notificationsApi.markAsRead(notificationId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: notificationKeys.lists() })
      queryClient.invalidateQueries({ queryKey: notificationKeys.unreadCount() })
    },
  })
}

export const useMarkAllAsRead = () => {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: () => notificationsApi.markAllAsRead(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: notificationKeys.lists() })
      queryClient.invalidateQueries({ queryKey: notificationKeys.unreadCount() })
    },
  })
}

export const useDeleteNotification = () => {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (notificationId: string) =>
      notificationsApi.deleteNotification(notificationId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: notificationKeys.lists() })
      queryClient.invalidateQueries({ queryKey: notificationKeys.unreadCount() })
    },
  })
}

export const useAdminNotifications = (params: UseNotificationsParams = {}) => {
  return useQuery({
    queryKey: notificationKeys.adminList(params),
    queryFn: () => notificationsApi.getAdminNotifications(params),
  })
}
