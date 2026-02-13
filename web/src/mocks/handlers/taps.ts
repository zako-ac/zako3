import { http, HttpResponse, delay } from 'msw'
import {
  allMockTaps,
  createTapWithAccess,
  createTapStats,
  createAuditLogForTap,
} from '../data'
import type {
  PaginatedResponse,
  TapWithAccess,
  TapFilters,
  TapSort,
  TapStats,
  CreateTapInput,
  UpdateTapInput,
  TapApiToken,
  TapApiTokenCreated,
  CreateTapApiTokenInput,
  UpdateTapApiTokenInput,
  TapApiTokenExpiry,
} from '@zako-ac/zako3-data'

const API_BASE = '/api'

const mockTapsStore = [...allMockTaps]

// API Tokens store
interface MockApiToken extends TapApiToken {
  fullToken?: string // Store full token for regeneration display
}

const mockApiTokensStore: MockApiToken[] = []

// Helper to generate mock tokens
const generateToken = (): string => {
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'
  let token = ''
  for (let i = 0; i < 64; i++) {
    token += chars.charAt(Math.floor(Math.random() * chars.length))
  }
  return `eyJ${token}` // JWT-like format
}

const maskToken = (token: string): string => {
  if (token.length <= 18) return token
  return `${token.slice(0, 12)}••••••••••${token.slice(-6)}`
}

const calculateExpiry = (expiryType: TapApiTokenExpiry): string | null => {
  if (expiryType === 'never') return null
  
  const now = new Date()
  switch (expiryType) {
    case '1_month':
      now.setMonth(now.getMonth() + 1)
      break
    case '3_months':
      now.setMonth(now.getMonth() + 3)
      break
    case '6_months':
      now.setMonth(now.getMonth() + 6)
      break
    case '1_year':
      now.setFullYear(now.getFullYear() + 1)
      break
  }
  return now.toISOString()
}


const applyFilters = (
  taps: TapWithAccess[],
  filters: TapFilters
): TapWithAccess[] => {
  let result = [...taps]

  if (filters.search) {
    const search = filters.search.toLowerCase()
    result = result.filter(
      (tap) =>
        tap.name.toLowerCase().includes(search) ||
        tap.description.toLowerCase().includes(search) ||
        tap.id.toLowerCase().includes(search)
    )
  }

  if (filters.roles && filters.roles.length > 0) {
    result = result.filter((tap) =>
      filters.roles!.some((role) => tap.roles.includes(role))
    )
  }

  if (filters.accessible !== undefined) {
    result = result.filter((tap) => tap.hasAccess === filters.accessible)
  }

  if (filters.ownerId) {
    result = result.filter((tap) => tap.ownerId === filters.ownerId)
  }

  return result
}

const applySort = (taps: TapWithAccess[], sort: TapSort): TapWithAccess[] => {
  const sorted = [...taps]

  sorted.sort((a, b) => {
    let comparison = 0

    switch (sort.field) {
      case 'mostUsed':
        comparison = b.totalUses - a.totalUses
        break
      case 'recentlyCreated':
        comparison =
          new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
        break
      case 'alphabetical':
        comparison = a.name.localeCompare(b.name)
        break
    }

    return sort.direction === 'desc' ? comparison : -comparison
  })

  return sorted
}

const paginate = <T>(
  items: T[],
  page: number,
  perPage: number
): PaginatedResponse<T> => {
  const total = items.length
  const totalPages = Math.ceil(total / perPage)
  const start = (page - 1) * perPage
  const end = start + perPage
  const data = items.slice(start, end)

  return {
    data,
    meta: {
      total,
      page,
      perPage,
      totalPages,
    },
  }
}

export const tapHandlers = [
  http.get(`${API_BASE}/taps`, async ({ request }) => {
    await delay(200)
    const url = new URL(request.url)

    const page = parseInt(url.searchParams.get('page') || '1')
    const perPage = parseInt(url.searchParams.get('perPage') || '20')
    const search = url.searchParams.get('search') || undefined
    const rolesParam = url.searchParams.get('roles')
    const roles = rolesParam
      ? (rolesParam.split(',') as TapFilters['roles'])
      : undefined
    const accessible =
      url.searchParams.get('accessible') === 'true'
        ? true
        : url.searchParams.get('accessible') === 'false'
          ? false
          : undefined
    const ownerId = url.searchParams.get('ownerId') || undefined
    const sortField =
      (url.searchParams.get('sortField') as TapSort['field']) ||
      'recentlyCreated'
    const sortDirection =
      (url.searchParams.get('sortDirection') as TapSort['direction']) || 'desc'

    let filtered = applyFilters(mockTapsStore, {
      search,
      roles,
      accessible,
      ownerId,
    })
    filtered = applySort(filtered, {
      field: sortField,
      direction: sortDirection,
    })
    const result = paginate(filtered, page, perPage)

    return HttpResponse.json(result)
  }),

  http.get(`${API_BASE}/taps/:tapId`, async ({ params }) => {
    await delay(100)
    const { tapId } = params
    const tap = mockTapsStore.find((t) => t.id === tapId)

    if (!tap) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Tap not found' },
        { status: 404 }
      )
    }

    return HttpResponse.json(tap)
  }),

  http.post(`${API_BASE}/taps`, async ({ request }) => {
    await delay(300)
    const body = (await request.json()) as CreateTapInput

    const existingTap = mockTapsStore.find((t) => t.id === body.id)
    if (existingTap) {
      return HttpResponse.json(
        { code: 'CONFLICT', message: 'Tap ID already exists' },
        { status: 409 }
      )
    }

    const newTap = createTapWithAccess({
      id: body.id,
      name: body.name,
      description: body.description,
      roles: body.roles,
      permission: body.permission,
      ownerId: 'current-user-id',
      occupation: 'base',
      hasAccess: true,
    })

    mockTapsStore.unshift(newTap)

    return HttpResponse.json(newTap, { status: 201 })
  }),

  http.patch(`${API_BASE}/taps/:tapId`, async ({ params, request }) => {
    await delay(200)
    const { tapId } = params
    const body = (await request.json()) as UpdateTapInput

    const tapIndex = mockTapsStore.findIndex((t) => t.id === tapId)
    if (tapIndex === -1) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Tap not found' },
        { status: 404 }
      )
    }

    if (body.id && body.id !== tapId) {
      const existingTap = mockTapsStore.find((t) => t.id === body.id)
      if (existingTap) {
        return HttpResponse.json(
          { code: 'CONFLICT', message: 'Tap ID already exists' },
          { status: 409 }
        )
      }
    }

    const updatedTap: TapWithAccess = {
      ...mockTapsStore[tapIndex],
      ...body,
      updatedAt: new Date().toISOString(),
    }

    mockTapsStore[tapIndex] = updatedTap

    return HttpResponse.json(updatedTap)
  }),

  http.delete(`${API_BASE}/taps/:tapId`, async ({ params }) => {
    await delay(200)
    const { tapId } = params

    const tapIndex = mockTapsStore.findIndex((t) => t.id === tapId)
    if (tapIndex === -1) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Tap not found' },
        { status: 404 }
      )
    }

    mockTapsStore.splice(tapIndex, 1)

    return new HttpResponse(null, { status: 204 })
  }),

  http.get(`${API_BASE}/taps/:tapId/stats`, async ({ params }) => {
    await delay(200)
    const { tapId } = params

    const tap = mockTapsStore.find((t) => t.id === tapId)
    if (!tap) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Tap not found' },
        { status: 404 }
      )
    }

    const stats: TapStats = createTapStats(tapId as string, {
      totalUses: tap.totalUses,
    })

    return HttpResponse.json(stats)
  }),

  http.get(`${API_BASE}/taps/:tapId/audit-log`, async ({ params, request }) => {
    await delay(200)
    const { tapId } = params
    const url = new URL(request.url)

    const page = parseInt(url.searchParams.get('page') || '1')
    const perPage = parseInt(url.searchParams.get('perPage') || '20')

    const tap = mockTapsStore.find((t) => t.id === tapId)
    if (!tap) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Tap not found' },
        { status: 404 }
      )
    }

    const auditLog = createAuditLogForTap(tapId as string, 50)
    const result = paginate(auditLog, page, perPage)

    return HttpResponse.json(result)
  }),

  http.post(`${API_BASE}/taps/:tapId/report`, async ({ params }) => {
    await delay(300)
    const { tapId } = params

    const tap = mockTapsStore.find((t) => t.id === tapId)
    if (!tap) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Tap not found' },
        { status: 404 }
      )
    }

    return new HttpResponse(null, { status: 204 })
  }),

  http.post(`${API_BASE}/taps/:tapId/verify`, async ({ params }) => {
    await delay(300)
    const { tapId } = params

    const tap = mockTapsStore.find((t) => t.id === tapId)
    if (!tap) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Tap not found' },
        { status: 404 }
      )
    }

    return new HttpResponse(null, { status: 204 })
  }),

  http.get(`${API_BASE}/users/me/taps`, async ({ request }) => {
    await delay(200)
    const url = new URL(request.url)

    const page = parseInt(url.searchParams.get('page') || '1')
    const perPage = parseInt(url.searchParams.get('perPage') || '20')

    const myTaps = mockTapsStore.filter((t) => t.ownerId === 'current-user-id')
    const result = paginate(myTaps, page, perPage)

    return HttpResponse.json(result)
  }),

  // API Token endpoints
  http.get(`${API_BASE}/taps/:tapId/api-tokens`, async ({ params }) => {
    await delay(150)
    const { tapId } = params

    const tap = mockTapsStore.find((t) => t.id === tapId)
    if (!tap) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Tap not found' },
        { status: 404 }
      )
    }

    const tokens = mockApiTokensStore
      .filter((t) => t.tapId === tapId)
      .map(({ fullToken, ...token }) => token) // Remove fullToken from response

    return HttpResponse.json(tokens)
  }),

  http.post(`${API_BASE}/taps/:tapId/api-tokens`, async ({ params, request }) => {
    await delay(250)
    const { tapId } = params
    const body = (await request.json()) as CreateTapApiTokenInput

    const tap = mockTapsStore.find((t) => t.id === tapId)
    if (!tap) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Tap not found' },
        { status: 404 }
      )
    }

    const fullToken = generateToken()
    const newToken: MockApiToken = {
      id: `token-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
      tapId: tapId as string,
      label: body.label,
      token: maskToken(fullToken),
      fullToken,
      createdAt: new Date().toISOString(),
      lastUsedAt: null,
      expiresAt: calculateExpiry(body.expiry),
    }

    mockApiTokensStore.push(newToken)

    const response: TapApiTokenCreated = {
      ...newToken,
      token: fullToken, // Return full token only on creation
    }
    delete (response as any).fullToken

    return HttpResponse.json(response, { status: 201 })
  }),

  http.patch(`${API_BASE}/taps/:tapId/api-tokens/:tokenId`, async ({ params, request }) => {
    await delay(200)
    const { tapId, tokenId } = params
    const body = (await request.json()) as UpdateTapApiTokenInput

    const tokenIndex = mockApiTokensStore.findIndex(
      (t) => t.id === tokenId && t.tapId === tapId
    )

    if (tokenIndex === -1) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Token not found' },
        { status: 404 }
      )
    }

    mockApiTokensStore[tokenIndex] = {
      ...mockApiTokensStore[tokenIndex],
      label: body.label,
    }

    const { fullToken, ...token } = mockApiTokensStore[tokenIndex]
    return HttpResponse.json(token)
  }),

  http.post(`${API_BASE}/taps/:tapId/api-tokens/:tokenId/regenerate`, async ({ params }) => {
    await delay(250)
    const { tapId, tokenId } = params

    const tokenIndex = mockApiTokensStore.findIndex(
      (t) => t.id === tokenId && t.tapId === tapId
    )

    if (tokenIndex === -1) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Token not found' },
        { status: 404 }
      )
    }

    const fullToken = generateToken()
    mockApiTokensStore[tokenIndex] = {
      ...mockApiTokensStore[tokenIndex],
      token: maskToken(fullToken),
      fullToken,
      createdAt: new Date().toISOString(),
      lastUsedAt: null,
    }

    const response: TapApiTokenCreated = {
      ...mockApiTokensStore[tokenIndex],
      token: fullToken, // Return full token
    }
    delete (response as any).fullToken

    return HttpResponse.json(response)
  }),

  http.delete(`${API_BASE}/taps/:tapId/api-tokens/:tokenId`, async ({ params }) => {
    await delay(200)
    const { tapId, tokenId } = params

    const tokenIndex = mockApiTokensStore.findIndex(
      (t) => t.id === tokenId && t.tapId === tapId
    )

    if (tokenIndex === -1) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Token not found' },
        { status: 404 }
      )
    }

    mockApiTokensStore.splice(tokenIndex, 1)

    return new HttpResponse(null, { status: 204 })
  }),
]
