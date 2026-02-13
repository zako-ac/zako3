import { faker } from '@faker-js/faker'
import type { User, UserWithActivity } from '@zako-ac/zako3-data'

export const createUser = (overrides?: Partial<User>): User => {
    const createdAt = faker.date.past({ years: 2 })
    const updatedAt = faker.date.between({ from: createdAt, to: new Date() })

    return {
        id: faker.string.uuid(),
        discordId: faker.string.numeric(18),
        username: "mincomk",
        avatar: "https://cdn.discordapp.com/avatars/700624937236561950/582e291f8269919dd710f41effe0020a.png?size=1024",
        email: faker.internet.email(),
        isAdmin: faker.datatype.boolean({ probability: 0.05 }),
        isBanned: faker.datatype.boolean({ probability: 0.02 }),
        banReason: undefined,
        banExpiresAt: undefined,
        createdAt: createdAt.toISOString(),
        updatedAt: updatedAt.toISOString(),
        ...overrides,
    }
}

export const createUserWithActivity = (
    overrides?: Partial<UserWithActivity>
): UserWithActivity => {
    const user = createUser(overrides)

    return {
        ...user,
        lastActiveAt: faker.date.recent({ days: 30 }).toISOString(),
        tapCount: faker.number.int({ min: 0, max: 20 }),
        totalTapUses: faker.number.int({ min: 0, max: 100000 }),
        ...overrides,
    }
}

export const mockCurrentUser = createUser({
    id: 'current-user-id',
    username: 'DemoUser',
    email: 'demo@example.com',
    isAdmin: false,
    isBanned: false,
})

export const mockAdminUser = createUser({
    id: 'current-user-id',
    username: 'AdminUser',
    email: 'admin@zako.ac',
    isAdmin: true,
    isBanned: false,
})

export const mockUsers: UserWithActivity[] = Array.from({ length: 100 }, () =>
    createUserWithActivity()
)

export const mockBannedUsers: UserWithActivity[] = Array.from({ length: 10 }, () =>
    createUserWithActivity({
        isBanned: true,
        banReason: faker.lorem.sentence(),
        banExpiresAt: faker.date.future({ years: 1 }).toISOString(),
    })
)

export const allMockUsers = [...mockUsers, ...mockBannedUsers]
