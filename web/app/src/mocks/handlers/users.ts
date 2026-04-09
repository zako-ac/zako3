import { http, HttpResponse, delay } from 'msw'
import { allMockUsers } from '../data/users'
import type {
  PaginatedResponse,
  UserWithActivity,
  UserFilters,
  UserSort,
  BanUserInput,
  UpdateUserRoleInput,
} from '@zako-ac/zako3-data'

const API_BASE = '/api'

const mockUsersStore = [...allMockUsers]

const applyFilters = (
  users: UserWithActivity[],
  filters: UserFilters
): UserWithActivity[] => {
  let result = [...users]

  if (filters.search) {
    const search = filters.search.toLowerCase()
    result = result.filter(
      (user) =>
        user.username.toLowerCase().includes(search) ||
        user.email?.toLowerCase().includes(search) ||
        user.discordId.includes(search)
    )
  }

  if (filters.isBanned !== undefined) {
    result = result.filter((user) => user.isBanned === filters.isBanned)
  }

  if (filters.isAdmin !== undefined) {
    result = result.filter((user) => user.isAdmin === filters.isAdmin)
  }

  return result
}

const applySort = (
  users: UserWithActivity[],
  sort: UserSort
): UserWithActivity[] => {
  const sorted = [...users]

  sorted.sort((a, b) => {
    let comparison = 0

    switch (sort.field) {
      case 'username':
        comparison = a.username.localeCompare(b.username)
        break
      case 'createdAt':
        comparison =
          new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
        break
      case 'lastActiveAt':
        comparison =
          new Date(b.lastActiveAt).getTime() - new Date(a.lastActiveAt).getTime()
        break
      case 'tapCount':
        comparison = b.tapCount - a.tapCount
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

export const userHandlers = [
  http.get(`${API_BASE}/admin/users`, async ({ request }) => {
    await delay(200)
    const url = new URL(request.url)

    const page = parseInt(url.searchParams.get('page') || '1')
    const perPage = parseInt(url.searchParams.get('perPage') || '20')
    const search = url.searchParams.get('search') || undefined
    const isBanned =
      url.searchParams.get('isBanned') === 'true'
        ? true
        : url.searchParams.get('isBanned') === 'false'
          ? false
          : undefined
    const isAdmin =
      url.searchParams.get('isAdmin') === 'true'
        ? true
        : url.searchParams.get('isAdmin') === 'false'
          ? false
          : undefined
    const sortField =
      (url.searchParams.get('sortField') as UserSort['field']) || 'createdAt'
    const sortDirection =
      (url.searchParams.get('sortDirection') as UserSort['direction']) || 'desc'

    let filtered = applyFilters(mockUsersStore, { search, isBanned, isAdmin })
    filtered = applySort(filtered, { field: sortField, direction: sortDirection })
    const result = paginate(filtered, page, perPage)

    return HttpResponse.json(result)
  }),

  http.get(`${API_BASE}/admin/users/:userId`, async ({ params }) => {
    await delay(100)
    const { userId } = params
    const user = mockUsersStore.find((u) => u.id === userId)

    if (!user) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'User not found' },
        { status: 404 }
      )
    }

    return HttpResponse.json(user)
  }),

  http.get(`${API_BASE}/users/:userId`, async ({ params }) => {
    await delay(100)
    const { userId } = params
    const user = mockUsersStore.find((u) => u.id === userId)

    if (!user) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'User not found' },
        { status: 404 }
      )
    }

    return HttpResponse.json({
      id: user.id,
      discordId: user.discordId,
      username: user.username,
      avatar: user.avatar,
      isAdmin: user.isAdmin,
      isBanned: user.isBanned,
      createdAt: user.createdAt,
    })
  }),

  http.post(`${API_BASE}/admin/users/:userId/ban`, async ({ params, request }) => {
    await delay(300)
    const { userId } = params
    const body = (await request.json()) as BanUserInput

    const userIndex = mockUsersStore.findIndex((u) => u.id === userId)
    if (userIndex === -1) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'User not found' },
        { status: 404 }
      )
    }

    mockUsersStore[userIndex] = {
      ...mockUsersStore[userIndex],
      isBanned: true,
      banReason: body.reason,
      banExpiresAt: body.expiresAt,
      updatedAt: new Date().toISOString(),
    }

    return HttpResponse.json(mockUsersStore[userIndex])
  }),

  http.post(`${API_BASE}/admin/users/:userId/unban`, async ({ params }) => {
    await delay(300)
    const { userId } = params

    const userIndex = mockUsersStore.findIndex((u) => u.id === userId)
    if (userIndex === -1) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'User not found' },
        { status: 404 }
      )
    }

    mockUsersStore[userIndex] = {
      ...mockUsersStore[userIndex],
      isBanned: false,
      banReason: undefined,
      banExpiresAt: undefined,
      updatedAt: new Date().toISOString(),
    }

    return HttpResponse.json(mockUsersStore[userIndex])
  }),

  http.patch(`${API_BASE}/admin/users/:userId/role`, async ({ params, request }) => {
    await delay(300)
    const { userId } = params
    const body = (await request.json()) as UpdateUserRoleInput

    const userIndex = mockUsersStore.findIndex((u) => u.id === userId)
    if (userIndex === -1) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'User not found' },
        { status: 404 }
      )
    }

    mockUsersStore[userIndex] = {
      ...mockUsersStore[userIndex],
      isAdmin: body.isAdmin,
      updatedAt: new Date().toISOString(),
    }

    return HttpResponse.json(mockUsersStore[userIndex])
  }),
]
