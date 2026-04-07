import { apiClient } from '@/lib/api-client'
import { apiCall } from '@/lib/api-helpers'

export interface MapperInputData {
    mapping_list?: true
    discord_info?: true
    caller_info?: true
    mapper_list?: true
}

export type MapperInputDataKey = 'mapping_list' | 'discord_info' | 'caller_info' | 'mapper_list'

export interface WasmMapperDto {
    id: string
    name: string
    wasm_filename: string
    sha256_hash: string
    input_data: MapperInputDataKey[]
}

export interface PipelineOrderDto {
    mapper_ids: string[]
}

export interface MapperStepResultDto {
    mapper_id: string
    mapper_name: string
    input_text: string
    output_text: string
    success: boolean
    error?: string
}

export interface EvaluateResultDto {
    final_text: string
    steps: MapperStepResultDto[]
}

export interface EvaluateRequestDto {
    text: string
    mapper_ids: string[]
}

export interface EvaluateSingleRequestDto {
    text: string
}

export const mappersApi = {
    listMappers: (): Promise<WasmMapperDto[]> =>
        apiCall(apiClient.get<WasmMapperDto[]>('/admin/mappers')),

    getMapper: (id: string): Promise<WasmMapperDto> =>
        apiCall(apiClient.get<WasmMapperDto>(`/admin/mappers/${encodeURIComponent(id)}`)),

    createMapper: (formData: FormData): Promise<WasmMapperDto> =>
        apiCall(apiClient.postFormData<WasmMapperDto>('/admin/mappers', formData)),

    updateMapper: (id: string, formData: FormData): Promise<WasmMapperDto> =>
        apiCall(apiClient.putFormData<WasmMapperDto>(`/admin/mappers/${encodeURIComponent(id)}`, formData)),

    deleteMapper: (id: string): Promise<void> =>
        apiCall(apiClient.delete<void>(`/admin/mappers/${encodeURIComponent(id)}`)),

    getPipeline: (): Promise<PipelineOrderDto> =>
        apiCall(apiClient.get<PipelineOrderDto>('/admin/mappers/pipeline')),

    setPipeline: (dto: PipelineOrderDto): Promise<void> =>
        apiCall(apiClient.put<void>('/admin/mappers/pipeline', dto)),

    evaluatePipeline: (req: EvaluateRequestDto): Promise<EvaluateResultDto> =>
        apiCall(apiClient.post<EvaluateResultDto>('/admin/mappers/evaluate', req)),

    evaluateMapper: (id: string, req: EvaluateSingleRequestDto): Promise<EvaluateResultDto> =>
        apiCall(apiClient.post<EvaluateResultDto>(`/admin/mappers/${encodeURIComponent(id)}/evaluate`, req)),
}
