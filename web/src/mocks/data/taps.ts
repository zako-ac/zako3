import { faker } from '@faker-js/faker'
import type {
  Tap,
  TapWithAccess,
  TapOccupation,
  TapRole,
  TapPermissionConfig,
  TapStats,
  TimeSeriesPoint,
  UserSummary,
} from '@zako-ac/zako3-data'

const TAP_OCCUPATIONS: TapOccupation[] = ['official', 'verified', 'base']
const TAP_ROLES: TapRole[] = ['music', 'tts']
const TAP_PERMISSION_TYPES = [
  'owner_only',
  'public',
  'whitelisted',
  'blacklisted',
] as const

export const createUserSummary = (
  overrides?: Partial<UserSummary>
): UserSummary => ({
  id: faker.string.uuid(),
  username: faker.internet.username(),
  avatar: faker.image.avatar(),
  ...overrides,
})

export const createTap = (overrides?: Partial<Tap>): Tap => {
  const createdAt = faker.date.past({ years: 1 })
  const updatedAt = faker.date.between({ from: createdAt, to: new Date() })

  // Generate permission config
  const permissionType =
    overrides?.permission?.type ||
    faker.helpers.arrayElement(TAP_PERMISSION_TYPES)

  let permission: TapPermissionConfig
  if (permissionType === 'owner_only') {
    permission = { type: 'owner_only' }
  } else if (permissionType === 'public') {
    permission = { type: 'public' }
  } else if (permissionType === 'whitelisted') {
    const userIds = Array.from(
      { length: faker.number.int({ min: 1, max: 10 }) },
      () => faker.string.uuid()
    )
    permission = { type: 'whitelisted', userIds }
  } else {
    const userIds = Array.from(
      { length: faker.number.int({ min: 1, max: 5 }) },
      () => faker.string.uuid()
    )
    permission = { type: 'blacklisted', userIds }
  }

  return {
    id: faker.internet
      .username()
      .toLowerCase()
      .replace(/[^a-z0-9_.]/g, '_'),
    name: faker.commerce.productName(),
    description: faker.lorem.paragraph(),
    createdAt: createdAt.toISOString(),
    updatedAt: updatedAt.toISOString(),
    ownerId: faker.string.uuid(),
    occupation: faker.helpers.arrayElement(TAP_OCCUPATIONS),
    roles: faker.helpers.arrayElements(TAP_ROLES, { min: 1, max: 2 }),
    permission,
    totalUses: faker.number.int({ min: 0, max: 10000 }),
    ...overrides,
  }
}

export const createTapWithAccess = (
  overrides?: Partial<TapWithAccess>
): TapWithAccess => {
  const tap = createTap(overrides)
  const owner = createUserSummary({ id: tap.ownerId })

  return {
    ...tap,
    hasAccess:
      tap.permission.type === 'public' ||
      faker.datatype.boolean({ probability: 0.7 }),
    owner,
    ...overrides,
  }
}

export const createTimeSeriesData = (
  days: number = 30,
  minValue: number = 0,
  maxValue: number = 100
): TimeSeriesPoint[] => {
  const data: TimeSeriesPoint[] = []
  const now = new Date()

  for (let i = days - 1; i >= 0; i--) {
    const date = new Date(now)
    date.setDate(date.getDate() - i)
    data.push({
      timestamp: date.toISOString(),
      value: faker.number.int({ min: minValue, max: maxValue }),
    })
  }

  return data
}

export const createTapStats = (
  tapId: string,
  overrides?: Partial<TapStats>
): TapStats => ({
  tapId,
  currentlyActive: faker.number.int({ min: 0, max: 50 }),
  totalUses: faker.number.int({ min: 100, max: 50000 }),
  cacheHits: faker.number.int({ min: 50, max: 40000 }),
  uniqueUsers: faker.number.int({ min: 10, max: 5000 }),
  useRateHistory: createTimeSeriesData(30, 0, 500),
  cacheHitRateHistory: createTimeSeriesData(30, 0, 100),
  ...overrides,
})

export const mockTaps: TapWithAccess[] = Array.from({ length: 50 }, () =>
  createTapWithAccess()
)

export const mockOfficialTaps: TapWithAccess[] = Array.from({ length: 5 }, () =>
  createTapWithAccess({ occupation: 'official' })
)

export const mockVerifiedTaps: TapWithAccess[] = Array.from(
  { length: 10 },
  () => createTapWithAccess({ occupation: 'verified' })
)

export const allMockTaps = [
  ...mockOfficialTaps,
  ...mockVerifiedTaps,
  ...mockTaps,
]
