import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { usersApi } from './api'
import type { UserFilters, UserSort, PaginationParams, BanUserInput, UpdateUserRoleInput } from '@zako-ac/zako3-data'

export const userKeys = {
    all: ['users'] as const,
    lists: () => [...userKeys.all, 'list'] as const,
    list: (filters: Record<string, unknown>) => [...userKeys.lists(), filters] as const,
    details: () => [...userKeys.all, 'detail'] as const,
    detail: (id: string) => [...userKeys.details(), id] as const,
    public: (id: string) => [...userKeys.all, 'public', id] as const,
}

type UseUsersParams = Partial<PaginationParams> & Partial<UserFilters> & {
    sortField?: UserSort['field']
    sortDirection?: UserSort['direction']
}

export const useUsers = (params: UseUsersParams = {}) => {
    return useQuery({
        queryKey: userKeys.list(params),
        queryFn: () => usersApi.getUsers(params),
    })
}

export const useUser = (userId: string | undefined) => {
    return useQuery({
        queryKey: userKeys.detail(userId!),
        queryFn: () => usersApi.getUser(userId!),
        enabled: !!userId,
    })
}

export const useUserPublic = (userId: string | undefined) => {
    return useQuery({
        queryKey: userKeys.public(userId!),
        queryFn: () => usersApi.getUserPublic(userId!),
        enabled: !!userId,
    })
}

export const useBanUser = () => {
    const queryClient = useQueryClient()

    return useMutation({
        mutationFn: ({ userId, ...data }: BanUserInput) => usersApi.banUser(userId, data),
        onSuccess: (updatedUser, { userId }) => {
            queryClient.setQueryData(userKeys.detail(userId), updatedUser)
            queryClient.invalidateQueries({ queryKey: userKeys.lists() })
        },
    })
}

export const useUnbanUser = () => {
    const queryClient = useQueryClient()

    return useMutation({
        mutationFn: (userId: string) => usersApi.unbanUser(userId),
        onSuccess: (updatedUser, userId) => {
            queryClient.setQueryData(userKeys.detail(userId), updatedUser)
            queryClient.invalidateQueries({ queryKey: userKeys.lists() })
        },
    })
}

export const useUpdateUserRole = () => {
    const queryClient = useQueryClient()

    return useMutation({
        mutationFn: ({ userId, ...data }: UpdateUserRoleInput) =>
            usersApi.updateUserRole(userId, data),
        onSuccess: (updatedUser, { userId }) => {
            queryClient.setQueryData(userKeys.detail(userId), updatedUser)
            queryClient.invalidateQueries({ queryKey: userKeys.lists() })
        },
    })
}
