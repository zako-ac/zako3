import { apiClient, buildQueryString } from '@/lib/api-client'
import { apiCall } from '@/lib/api-helpers'
import type {
  PaginatedResponse,
  PaginationParams,
  Notification,
  NotificationFilters,
  NotificationSort,
} from '@zako-ac/zako3-data'

interface GetNotificationsParams
  extends Partial<PaginationParams>, Partial<NotificationFilters> {
  sortField?: NotificationSort['field']
  sortDirection?: NotificationSort['direction']
}

export const notificationsApi = {
  getNotifications: async (
    params: GetNotificationsParams = {}
  ): Promise<PaginatedResponse<Notification>> => {
    const query = buildQueryString({
      page: params.page,
      perPage: params.perPage,
      search: params.search,
      level: params.level,
      category: params.category,
      isRead: params.isRead,
      sortField: params.sortField,
      sortDirection: params.sortDirection,
    })
    return apiCall(
      apiClient.get<PaginatedResponse<Notification>>(`/notifications${query}`)
    )
  },

  getUnreadCount: async (): Promise<{ count: number }> => {
    return apiCall(
      apiClient.get<{ count: number }>('/notifications/unread-count')
    )
  },

  markAsRead: async (notificationId: string): Promise<Notification> => {
    return apiCall(
      apiClient.patch<Notification>(`/notifications/${notificationId}/read`)
    )
  },

  markAllAsRead: async (): Promise<void> => {
    return apiCall(apiClient.patch('/notifications/read-all'))
  },

  deleteNotification: async (notificationId: string): Promise<void> => {
    return apiCall(apiClient.delete(`/notifications/${notificationId}`))
  },

  getAdminNotifications: async (
    params: GetNotificationsParams = {}
  ): Promise<PaginatedResponse<Notification>> => {
    const query = buildQueryString({
      page: params.page,
      perPage: params.perPage,
      search: params.search,
      level: params.level,
      category: params.category,
      sortField: params.sortField,
      sortDirection: params.sortDirection,
    })
    return apiCall(
      apiClient.get<PaginatedResponse<Notification>>(
        `/admin/notifications${query}`
      )
    )
  },
}
