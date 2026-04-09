import { apiClient, buildQueryString } from '@/lib/api-client'
import { apiCall } from '@/lib/api-helpers'
import type {
  PaginatedResponse,
  PaginationParams,
  User,
  UserWithActivity,
  UserFilters,
  UserSort,
  BanUserInput,
  UpdateUserRoleInput,
} from '@zako-ac/zako3-data'

interface GetUsersParams
  extends Partial<PaginationParams>, Partial<UserFilters> {
  sortField?: UserSort['field']
  sortDirection?: UserSort['direction']
}

export const usersApi = {
  getUsers: async (
    params: GetUsersParams = {}
  ): Promise<PaginatedResponse<UserWithActivity>> => {
    const query = buildQueryString({
      page: params.page,
      perPage: params.perPage,
      search: params.search,
      isBanned: params.isBanned,
      isAdmin: params.isAdmin,
      sortField: params.sortField,
      sortDirection: params.sortDirection,
    })
    return apiCall(
      apiClient.get<PaginatedResponse<UserWithActivity>>(`/admin/users${query}`)
    )
  },

  getUser: async (userId: string): Promise<UserWithActivity> => {
    return apiCall(apiClient.get<UserWithActivity>(`/admin/users/${userId}`))
  },

  getUserPublic: async (userId: string): Promise<User> => {
    return apiCall(apiClient.get<User>(`/users/${userId}`))
  },

  banUser: async (
    userId: string,
    data: Omit<BanUserInput, 'userId'>
  ): Promise<UserWithActivity> => {
    return apiCall(
      apiClient.post<UserWithActivity>(`/admin/users/${userId}/ban`, data)
    )
  },

  unbanUser: async (userId: string): Promise<UserWithActivity> => {
    return apiCall(
      apiClient.post<UserWithActivity>(`/admin/users/${userId}/unban`)
    )
  },

  updateUserRole: async (
    userId: string,
    data: Omit<UpdateUserRoleInput, 'userId'>
  ): Promise<UserWithActivity> => {
    return apiCall(
      apiClient.patch<UserWithActivity>(`/admin/users/${userId}/role`, data)
    )
  },
}
