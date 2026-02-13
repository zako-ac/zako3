import { faker } from '@faker-js/faker'
import type {
    Notification,
    NotificationCategory,
    NotificationLevel,
    AuditLogEntry,
    TapAuditLogEntry,
} from '@zako-ac/zako3-data'

const NOTIFICATION_LEVELS: NotificationLevel[] = ['info', 'success', 'warning', 'error']

const NOTIFICATION_CATEGORIES: NotificationCategory[] = [
    'tap_created',
    'tap_updated',
    'tap_deleted',
    'tap_reported',
    'tap_verified',
    'tap_verification_requested',
    'tap_verification_approved',
    'tap_verification_rejected',
    'user_banned',
    'user_unbanned',
    'user_role_changed',
    'system_alert',
]

const CATEGORY_TITLES: Record<NotificationCategory, string> = {
    tap_created: 'New Tap Created',
    tap_updated: 'Tap Updated',
    tap_deleted: 'Tap Deleted',
    tap_reported: 'Tap Reported',
    tap_verified: 'Tap Verified',
    tap_verification_requested: 'Verification Requested',
    tap_verification_approved: 'Verification Approved',
    tap_verification_rejected: 'Verification Rejected',
    user_banned: 'User Banned',
    user_unbanned: 'User Unbanned',
    user_role_changed: 'User Role Changed',
    system_alert: 'System Alert',
    custom: 'Notification',
}

const CATEGORY_LEVELS: Record<NotificationCategory, NotificationLevel> = {
    tap_created: 'info',
    tap_updated: 'info',
    tap_deleted: 'warning',
    tap_reported: 'warning',
    tap_verified: 'success',
    tap_verification_requested: 'info',
    tap_verification_approved: 'success',
    tap_verification_rejected: 'warning',
    user_banned: 'error',
    user_unbanned: 'success',
    user_role_changed: 'info',
    system_alert: 'warning',
    custom: 'info',
}

export const createNotification = (
    overrides?: Partial<Notification>
): Notification => {
    const category = faker.helpers.arrayElement(NOTIFICATION_CATEGORIES)
    const level = CATEGORY_LEVELS[category]

    return {
        id: faker.string.uuid(),
        userId: faker.string.uuid(),
        category,
        level,
        title: CATEGORY_TITLES[category],
        message: faker.lorem.sentence(),
        metadata: {},
        isRead: faker.datatype.boolean({ probability: 0.6 }),
        createdAt: faker.date.recent({ days: 30 }).toISOString(),
        ...overrides,
    }
}

export const createAuditLogEntry = (
    tapId: string,
    overrides?: Partial<AuditLogEntry>
): AuditLogEntry => ({
    id: faker.string.uuid(),
    tapId,
    actorId: faker.string.uuid(),
    action: faker.helpers.arrayElement([
        'tap.created',
        'tap.updated',
        'tap.settings.changed',
        'tap.permission.changed',
        'tap.role.added',
        'tap.role.removed',
        'tap.verification.requested',
        'tap.verification.approved',
        'tap.verification.rejected',
    ]),
    level: faker.helpers.arrayElement(NOTIFICATION_LEVELS),
    details: {
        field: faker.helpers.arrayElement(['name', 'description', 'permission', 'roles']),
        oldValue: faker.word.sample(),
        newValue: faker.word.sample(),
    },
    createdAt: faker.date.recent({ days: 90 }).toISOString(),
    ...overrides,
})

export const createTapAuditLogEntry = (
    tapId: string,
    overrides?: Partial<TapAuditLogEntry>
): TapAuditLogEntry => ({
    id: faker.string.uuid(),
    tapId,
    actorId: faker.string.uuid(),
    action: faker.helpers.arrayElement([
        'tap.created',
        'tap.updated',
        'tap.settings.changed',
        'tap.permission.changed',
        'tap.role.added',
        'tap.role.removed',
        'tap.verification.requested',
    ]),
    details: faker.lorem.sentence(),
    createdAt: faker.date.recent({ days: 90 }).toISOString(),
    ...overrides,
})

export const mockNotifications: Notification[] = Array.from({ length: 50 }, () =>
    createNotification()
)

export const mockUnreadNotifications = mockNotifications.filter((n) => !n.isRead)

export const createAuditLogForTap = (
    tapId: string,
    count: number = 20
): TapAuditLogEntry[] =>
    Array.from({ length: count }, () => createTapAuditLogEntry(tapId))
