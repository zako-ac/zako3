import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
    mappersApi,
    type WasmMapperDto,
    type PipelineOrderDto,
    type EvaluateRequestDto,
    type EvaluateSingleRequestDto,
} from './api'

export const mapperKeys = {
    all: ['mappers'] as const,
    lists: () => [...mapperKeys.all, 'list'] as const,
    detail: (id: string) => [...mapperKeys.all, 'detail', id] as const,
    pipeline: () => [...mapperKeys.all, 'pipeline'] as const,
}

export const useMappers = () =>
    useQuery({
        queryKey: mapperKeys.lists(),
        queryFn: () => mappersApi.listMappers(),
    })

export const useMapper = (id: string) =>
    useQuery({
        queryKey: mapperKeys.detail(id),
        queryFn: () => mappersApi.getMapper(id),
        enabled: !!id,
    })

export const usePipeline = () =>
    useQuery({
        queryKey: mapperKeys.pipeline(),
        queryFn: () => mappersApi.getPipeline(),
    })

export const useCreateMapper = () => {
    const queryClient = useQueryClient()
    return useMutation({
        mutationFn: (formData: FormData) => mappersApi.createMapper(formData),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: mapperKeys.lists() })
        },
    })
}

export const useUpdateMapper = (id: string) => {
    const queryClient = useQueryClient()
    return useMutation({
        mutationFn: (formData: FormData) => mappersApi.updateMapper(id, formData),
        onSuccess: (updated: WasmMapperDto) => {
            queryClient.setQueryData(mapperKeys.detail(id), updated)
            queryClient.invalidateQueries({ queryKey: mapperKeys.lists() })
        },
    })
}

export const useDeleteMapper = () => {
    const queryClient = useQueryClient()
    return useMutation({
        mutationFn: (id: string) => mappersApi.deleteMapper(id),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: mapperKeys.lists() })
            queryClient.invalidateQueries({ queryKey: mapperKeys.pipeline() })
        },
    })
}

export const useSetPipeline = () => {
    const queryClient = useQueryClient()
    return useMutation({
        mutationFn: (dto: PipelineOrderDto) => mappersApi.setPipeline(dto),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: mapperKeys.pipeline() })
        },
    })
}

export const useEvaluatePipeline = () =>
    useMutation({
        mutationFn: (req: EvaluateRequestDto) => mappersApi.evaluatePipeline(req),
    })

export const useEvaluateMapper = () =>
    useMutation({
        mutationFn: ({ id, text }: { id: string; text: string }) =>
            mappersApi.evaluateMapper(id, { text } satisfies EvaluateSingleRequestDto),
    })
