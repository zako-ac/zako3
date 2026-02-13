import { http, HttpResponse, delay } from 'msw'
import { mockNotifications } from '../data/notifications'
import type {
  PaginatedResponse,
  Notification,
  NotificationFilters,
  NotificationSort,
} from '@zako-ac/zako3-data'

const API_BASE = '/api'

const mockNotificationsStore = [...mockNotifications]

const applyFilters = (
  notifications: Notification[],
  filters: NotificationFilters
): Notification[] => {
  let result = [...notifications]

  if (filters.search) {
    const search = filters.search.toLowerCase()
    result = result.filter(
      (notification) =>
        notification.title.toLowerCase().includes(search) ||
        notification.message.toLowerCase().includes(search)
    )
  }

  if (filters.level) {
    result = result.filter((notification) => notification.level === filters.level)
  }

  if (filters.category) {
    result = result.filter(
      (notification) => notification.category === filters.category
    )
  }

  if (filters.isRead !== undefined) {
    result = result.filter((notification) => notification.isRead === filters.isRead)
  }

  return result
}

const applySort = (
  notifications: Notification[],
  sort: NotificationSort
): Notification[] => {
  const sorted = [...notifications]

  sorted.sort((a, b) => {
    let comparison = 0

    switch (sort.field) {
      case 'createdAt':
        comparison =
          new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
        break
      case 'level': {
        const levelOrder = { error: 0, warning: 1, success: 2, info: 3 }
        comparison = levelOrder[a.level] - levelOrder[b.level]
        break
      }
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

export const notificationHandlers = [
  http.get(`${API_BASE}/notifications`, async ({ request }) => {
    await delay(200)
    const url = new URL(request.url)

    const page = parseInt(url.searchParams.get('page') || '1')
    const perPage = parseInt(url.searchParams.get('perPage') || '20')
    const search = url.searchParams.get('search') || undefined
    const level = url.searchParams.get('level') as NotificationFilters['level']
    const category =
      url.searchParams.get('category') as NotificationFilters['category']
    const isRead =
      url.searchParams.get('isRead') === 'true'
        ? true
        : url.searchParams.get('isRead') === 'false'
          ? false
          : undefined
    const sortField =
      (url.searchParams.get('sortField') as NotificationSort['field']) ||
      'createdAt'
    const sortDirection =
      (url.searchParams.get('sortDirection') as NotificationSort['direction']) ||
      'desc'

    let filtered = applyFilters(mockNotificationsStore, {
      search,
      level,
      category,
      isRead,
    })
    filtered = applySort(filtered, { field: sortField, direction: sortDirection })
    const result = paginate(filtered, page, perPage)

    return HttpResponse.json(result)
  }),

  http.get(`${API_BASE}/notifications/unread-count`, async () => {
    await delay(100)
    const unreadCount = mockNotificationsStore.filter((n) => !n.isRead).length
    return HttpResponse.json({ count: unreadCount })
  }),

  http.patch(`${API_BASE}/notifications/:notificationId/read`, async ({ params }) => {
    await delay(100)
    const { notificationId } = params

    const notificationIndex = mockNotificationsStore.findIndex(
      (n) => n.id === notificationId
    )
    if (notificationIndex === -1) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Notification not found' },
        { status: 404 }
      )
    }

    mockNotificationsStore[notificationIndex] = {
      ...mockNotificationsStore[notificationIndex],
      isRead: true,
    }

    return HttpResponse.json(mockNotificationsStore[notificationIndex])
  }),

  http.patch(`${API_BASE}/notifications/read-all`, async () => {
    await delay(200)

    mockNotificationsStore.forEach((notification, index) => {
      mockNotificationsStore[index] = { ...notification, isRead: true }
    })

    return new HttpResponse(null, { status: 204 })
  }),

  http.delete(`${API_BASE}/notifications/:notificationId`, async ({ params }) => {
    await delay(100)
    const { notificationId } = params

    const notificationIndex = mockNotificationsStore.findIndex(
      (n) => n.id === notificationId
    )
    if (notificationIndex === -1) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Notification not found' },
        { status: 404 }
      )
    }

    mockNotificationsStore.splice(notificationIndex, 1)

    return new HttpResponse(null, { status: 204 })
  }),

  http.get(`${API_BASE}/admin/notifications`, async ({ request }) => {
    await delay(200)
    const url = new URL(request.url)

    const page = parseInt(url.searchParams.get('page') || '1')
    const perPage = parseInt(url.searchParams.get('perPage') || '20')
    const search = url.searchParams.get('search') || undefined
    const level = url.searchParams.get('level') as NotificationFilters['level']
    const category =
      url.searchParams.get('category') as NotificationFilters['category']
    const sortField =
      (url.searchParams.get('sortField') as NotificationSort['field']) ||
      'createdAt'
    const sortDirection =
      (url.searchParams.get('sortDirection') as NotificationSort['direction']) ||
      'desc'

    let filtered = applyFilters(mockNotificationsStore, {
      search,
      level,
      category,
    })
    filtered = applySort(filtered, { field: sortField, direction: sortDirection })
    const result = paginate(filtered, page, perPage)

    return HttpResponse.json(result)
  }),
]
