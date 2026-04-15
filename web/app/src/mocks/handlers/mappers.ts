import { http, HttpResponse, delay } from 'msw'
import { faker } from '@faker-js/faker'
import { API_BASE } from './base'
import type {
  WasmMapperDto,
  PipelineOrderDto,
  EvaluateRequestDto,
  EvaluateSingleRequestDto,
  MapperStepResultDto,
} from '@/features/mappers/api'

interface MockMapper extends WasmMapperDto {
  createdAt: string
}

const mockMappers: MockMapper[] = [
  {
    id: 'mapper_1',
    name: 'Text Normalizer',
    wasm_filename: 'text_normalizer.wasm',
    sha256_hash: 'abc123def456',
    input_data: ['mapping_list'],
    createdAt: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString(),
  },
  {
    id: 'mapper_2',
    name: 'Emoji Replacer',
    wasm_filename: 'emoji_replacer.wasm',
    sha256_hash: 'ghi789jkl012',
    input_data: ['mapper_list'],
    createdAt: new Date(Date.now() - 3 * 24 * 60 * 60 * 1000).toISOString(),
  },
  {
    id: 'mapper_3',
    name: 'Custom Text Filter',
    wasm_filename: 'text_filter.wasm',
    sha256_hash: 'mno345pqr678',
    input_data: ['mapping_list'],
    createdAt: new Date().toISOString(),
  },
]

let mockPipeline: PipelineOrderDto = {
  mapper_ids: ['mapper_1', 'mapper_2'],
}

export const mappersHandlers = [
  http.get(`${API_BASE}/admin/mappers`, async () => {
    await delay(200)
    return HttpResponse.json(mockMappers)
  }),

  http.get(`${API_BASE}/admin/mappers/:id`, async ({ params }) => {
    await delay(100)
    const { id } = params
    const mapper = mockMappers.find((m) => m.id === id)

    if (!mapper) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Mapper not found' },
        { status: 404 }
      )
    }

    return HttpResponse.json(mapper)
  }),

  http.post(`${API_BASE}/admin/mappers`, async ({ request }) => {
    await delay(300)
    const formData = await request.formData()
    const nameValue = formData.get('name')
    const fileValue = formData.get('file')

    if (!nameValue || !fileValue) {
      return HttpResponse.json(
        { code: 'INVALID_INPUT', message: 'name and file are required' },
        { status: 400 }
      )
    }

    const newMapper: MockMapper = {
      id: `mapper_${Date.now()}`,
      name: String(nameValue),
      wasm_filename: (fileValue as File).name,
      sha256_hash: faker.string.hexadecimal({ length: 32 }),
      input_data: ['mapping_list'],
      createdAt: new Date().toISOString(),
    }

    mockMappers.push(newMapper)

    return HttpResponse.json(newMapper, { status: 201 })
  }),

  http.put(`${API_BASE}/admin/mappers/:id`, async ({ params, request }) => {
    await delay(300)
    const { id } = params
    const mapperIndex = mockMappers.findIndex((m) => m.id === id)

    if (mapperIndex === -1) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Mapper not found' },
        { status: 404 }
      )
    }

    const formData = await request.formData()
    const nameValue = formData.get('name')

    if (nameValue) {
      mockMappers[mapperIndex].name = String(nameValue)
    }

    return HttpResponse.json(mockMappers[mapperIndex])
  }),

  http.delete(`${API_BASE}/admin/mappers/:id`, async ({ params }) => {
    await delay(200)
    const { id } = params
    const mapperIndex = mockMappers.findIndex((m) => m.id === id)

    if (mapperIndex === -1) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Mapper not found' },
        { status: 404 }
      )
    }

    mockMappers.splice(mapperIndex, 1)
    // Remove from pipeline if present
    mockPipeline.mapper_ids = mockPipeline.mapper_ids.filter((mid) => mid !== id)

    return new HttpResponse(null, { status: 204 })
  }),

  http.get(`${API_BASE}/admin/mappers/pipeline`, async () => {
    await delay(100)
    return HttpResponse.json(mockPipeline)
  }),

  http.put(`${API_BASE}/admin/mappers/pipeline`, async ({ request }) => {
    await delay(150)
    const body = (await request.json()) as PipelineOrderDto
    mockPipeline = body
    return HttpResponse.json(mockPipeline)
  }),

  http.post(`${API_BASE}/admin/mappers/evaluate`, async ({ request }) => {
    await delay(300)
    const body = (await request.json()) as EvaluateRequestDto

    const steps: MapperStepResultDto[] = body.mapper_ids.map((mapperId) => {
      const mapper = mockMappers.find((m) => m.id === mapperId)
      return {
        mapper_id: mapperId,
        mapper_name: mapper?.name || 'Unknown',
        input_text: body.text,
        output_text: body.text, // Passthrough for mock
        success: true,
      }
    })

    return HttpResponse.json({
      final_text: body.text,
      steps,
    })
  }),

  http.post(`${API_BASE}/admin/mappers/:id/evaluate`, async ({ params, request }) => {
    await delay(300)
    const { id } = params
    const mapper = mockMappers.find((m) => m.id === id)

    if (!mapper) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Mapper not found' },
        { status: 404 }
      )
    }

    const body = (await request.json()) as EvaluateSingleRequestDto

    return HttpResponse.json({
      final_text: body.text,
      steps: [
        {
          mapper_id: id,
          mapper_name: mapper.name,
          input_text: body.text,
          output_text: body.text, // Passthrough for mock
          success: true,
        },
      ],
    })
  }),
]
