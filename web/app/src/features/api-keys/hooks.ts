import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { apiKeysApi } from './api'
import type { CreateUserApiKeyInput, UpdateUserApiKeyInput } from '@zako-ac/zako3-data'

export const apiKeyKeys = {
    all: ['api-keys'] as const,
    lists: () => [...apiKeyKeys.all, 'list'] as const,
}

export const useApiKeys = () => {
    return useQuery({
        queryKey: apiKeyKeys.lists(),
        queryFn: () => apiKeysApi.list(),
    })
}

export const useCreateApiKey = () => {
    const queryClient = useQueryClient()

    return useMutation({
        mutationFn: (data: CreateUserApiKeyInput) => apiKeysApi.create(data),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: apiKeyKeys.lists() })
        },
    })
}

export const useUpdateApiKey = () => {
    const queryClient = useQueryClient()

    return useMutation({
        mutationFn: ({ keyId, data }: { keyId: string; data: UpdateUserApiKeyInput }) =>
            apiKeysApi.update(keyId, data),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: apiKeyKeys.lists() })
        },
    })
}

export const useRevokeApiKey = () => {
    const queryClient = useQueryClient()

    return useMutation({
        mutationFn: (keyId: string) => apiKeysApi.revoke(keyId),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: apiKeyKeys.lists() })
        },
    })
}
