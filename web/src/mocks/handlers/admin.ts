import { http, HttpResponse, delay } from 'msw'
import { faker } from '@faker-js/faker'
import type {
  AdminActivity,
  PaginatedResponse,
  VerificationRequestFull,
  VerificationStatus,
} from '@zako-ac/zako3-data'
import { createTapWithAccess } from '../data'

const API_BASE = '/api'

// Mock admin activity data
const generateMockActivity = (): AdminActivity => ({
  id: faker.string.uuid(),
  adminId: faker.string.uuid(),
  adminUsername: faker.internet.email(),
  action: faker.helpers.arrayElement([
    'ban_user',
    'unban_user',
    'delete_tap',
    'approve_tap',
    'reject_verification',
  ]),
  targetType: faker.helpers.arrayElement([
    'user',
    'tap',
    'notification',
    'system',
  ]),
  targetId: faker.string.uuid(),
  targetName: faker.helpers.arrayElement([
    'user_' + faker.person.firstName(),
    'tap_' + faker.lorem.word(),
    'system_config',
  ]),
  timestamp: faker.date.recent({ days: 7 }).toISOString(),
  details: faker.lorem.sentence(),
})

const mockActivity = Array.from({ length: 50 }, generateMockActivity).sort(
  (a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
)

// Mock verification request data
const generateMockVerificationRequest = (
  status?: VerificationStatus
): VerificationRequestFull => {
  const requestedAt = faker.date.recent({ days: 30 })
  const tap = createTapWithAccess({ occupation: 'base' })
  const requestStatus =
    status ||
    faker.helpers.arrayElement<VerificationStatus>([
      'pending',
      'approved',
      'rejected',
    ])

  const request: VerificationRequestFull = {
    id: faker.string.uuid(),
    tapId: tap.id,
    tap,
    reason: faker.helpers.arrayElement([
      'This tap is used by a verified organization',
      'Official tap for our company',
      'Widely used tap with substantial user base',
      'Educational institution official tap',
      'Government agency official tap',
      'Non-profit organization verified account',
    ]),
    evidence: faker.datatype.boolean({ probability: 0.6 })
      ? faker.internet.url()
      : undefined,
    status: requestStatus,
    requestedAt: requestedAt.toISOString(),
    reviewedAt:
      requestStatus !== 'pending'
        ? faker.date
            .between({ from: requestedAt, to: new Date() })
            .toISOString()
        : undefined,
    reviewedBy:
      requestStatus !== 'pending' ? faker.internet.email() : undefined,
    rejectionReason:
      requestStatus === 'rejected'
        ? faker.helpers.arrayElement([
            'Insufficient evidence provided',
            'Does not meet verification criteria',
            'Unable to verify organization credentials',
            'Duplicate verification request',
          ])
        : undefined,
  }

  return request
}

// Generate mix of verification requests
const mockVerificationRequests: VerificationRequestFull[] = [
  ...Array.from({ length: 8 }, () =>
    generateMockVerificationRequest('pending')
  ),
  ...Array.from({ length: 15 }, () =>
    generateMockVerificationRequest('approved')
  ),
  ...Array.from({ length: 7 }, () =>
    generateMockVerificationRequest('rejected')
  ),
].sort(
  (a, b) =>
    new Date(b.requestedAt).getTime() - new Date(a.requestedAt).getTime()
)

export const adminHandlers = [
  // Get admin activity log
  http.get(`${API_BASE}/admin/activity`, async ({ request }) => {
    await delay(200)
    const url = new URL(request.url)
    const page = parseInt(url.searchParams.get('page') || '1')
    const perPage = parseInt(url.searchParams.get('perPage') || '20')

    const total = mockActivity.length
    const totalPages = Math.ceil(total / perPage)
    const start = (page - 1) * perPage
    const end = start + perPage
    const data = mockActivity.slice(start, end)

    const result: PaginatedResponse<AdminActivity> = {
      data,
      meta: {
        total,
        page,
        perPage,
        totalPages,
      },
    }

    return HttpResponse.json(result)
  }),

  // Get pending verification requests
  http.get(`${API_BASE}/admin/taps/pending-verification`, async () => {
    await delay(200)
    // Return empty array for now - can be populated with mock pending taps if needed
    return HttpResponse.json([])
  }),

  // Get all verification requests (paginated, filtered)
  http.get(`${API_BASE}/admin/verifications`, async ({ request }) => {
    await delay(200)
    const url = new URL(request.url)
    const page = parseInt(url.searchParams.get('page') || '1')
    const perPage = parseInt(url.searchParams.get('perPage') || '20')
    const statusFilter = url.searchParams.get('status') as
      | VerificationStatus
      | 'all'
      | null

    // Filter by status if specified
    let filteredRequests = [...mockVerificationRequests]
    if (statusFilter && statusFilter !== 'all') {
      filteredRequests = filteredRequests.filter(
        (req) => req.status === statusFilter
      )
    }

    const total = filteredRequests.length
    const totalPages = Math.ceil(total / perPage)
    const start = (page - 1) * perPage
    const end = start + perPage
    const data = filteredRequests.slice(start, end)

    const result: PaginatedResponse<VerificationRequestFull> = {
      data,
      meta: {
        total,
        page,
        perPage,
        totalPages,
      },
    }

    return HttpResponse.json(result)
  }),

  // Approve verification request
  http.post(
    `${API_BASE}/admin/verifications/:id/approve`,
    async ({ params }) => {
      await delay(300)
      const { id } = params

      const requestIndex = mockVerificationRequests.findIndex(
        (req) => req.id === id
      )

      if (requestIndex === -1) {
        return HttpResponse.json(
          { error: 'Verification request not found' },
          { status: 404 }
        )
      }

      const request = mockVerificationRequests[requestIndex]

      if (request.status !== 'pending') {
        return HttpResponse.json(
          { error: 'Request has already been reviewed' },
          { status: 400 }
        )
      }

      // Update the request
      mockVerificationRequests[requestIndex] = {
        ...request,
        status: 'approved',
        reviewedAt: new Date().toISOString(),
        reviewedBy: 'admin@example.com', // In real app, this would be the authenticated admin
      }

      // Also update the tap occupation to 'verified'
      mockVerificationRequests[requestIndex].tap.occupation = 'verified'

      return HttpResponse.json(mockVerificationRequests[requestIndex])
    }
  ),

  // Reject verification request
  http.post(
    `${API_BASE}/admin/verifications/:id/reject`,
    async ({ params, request }) => {
      await delay(300)
      const { id } = params
      const body = (await request.json()) as { reason: string }

      const requestIndex = mockVerificationRequests.findIndex(
        (req) => req.id === id
      )

      if (requestIndex === -1) {
        return HttpResponse.json(
          { error: 'Verification request not found' },
          { status: 404 }
        )
      }

      const verificationRequest = mockVerificationRequests[requestIndex]

      if (verificationRequest.status !== 'pending') {
        return HttpResponse.json(
          { error: 'Request has already been reviewed' },
          { status: 400 }
        )
      }

      if (!body.reason) {
        return HttpResponse.json(
          { error: 'Rejection reason is required' },
          { status: 400 }
        )
      }

      // Update the request
      mockVerificationRequests[requestIndex] = {
        ...verificationRequest,
        status: 'rejected',
        reviewedAt: new Date().toISOString(),
        reviewedBy: 'admin@example.com', // In real app, this would be the authenticated admin
        rejectionReason: body.reason,
      }

      return HttpResponse.json(mockVerificationRequests[requestIndex])
    }
  ),
]
