/**
 * Mock dependencies for OpenAPI spec generation
 * These mocks satisfy the type requirements without needing real database/Redis connections
 */

import type { Logger } from "pino";
import type { Database } from "../src/infra/database.js";
import type { RedisClient } from "../src/infra/redis.js";
import type { SettingsService } from "../src/service/settings/index.js";
import type { SettingsOperations } from "../src/service/settings/index.js";
import type { UserService } from "../src/services/user.service.js";
import type { TapService } from "../src/services/tap.service.js";
import type { Result } from "../src/lib/result.js";

/**
 * Mock Logger - outputs nothing
 */
export const mockLogger: Logger = {
    fatal: () => {},
    error: () => {},
    warn: () => {},
    info: () => {},
    debug: () => {},
    trace: () => {},
    silent: () => {},
    child: () => mockLogger,
    level: "silent",
    bindings: () => ({}),
    flush: () => {},
} as any;

/**
 * Mock Database
 */
export const mockDatabase: Database = {
    client: {} as any,
    close: async () => {},
    isHealthy: async () => true,
} as any;

/**
 * Mock Redis Client
 */
export const mockRedis: RedisClient = {
    client: {} as any,
    close: async () => {},
    isHealthy: async () => true,
};

/**
 * Mock Settings Service
 */
export const mockSettingsService: SettingsService = {
    initialize: async () => ({ ok: true, value: undefined }) as any,
    shutdown: async () => {},
    isHealthy: async () => true,
    get: async () => ({ ok: true, value: { value: null, source: {} } }) as any,
    set: async () => ({ ok: true, value: undefined }) as any,
    delete: async () => ({ ok: true, value: true }) as any,
    list: async () => ({ ok: true, value: [] }) as any,
} as any;

/**
 * Mock Settings Operations
 */
export const mockSettingsOperations: SettingsOperations = {
    get: async () => ({ ok: true, value: { value: null, source: {} } }) as any,
    set: async () => ({ ok: true, value: undefined }) as any,
    delete: async () => ({ ok: true, value: true }) as any,
    list: async () => ({ ok: true, value: [] }) as any,
} as any;

/**
 * Mock User Service
 */
export const mockUserService: UserService = {
    getUser: async () =>
        ({
            ok: true,
            value: {
                id: "mock",
                username: "mock",
                isBanned: false,
                createdAt: new Date().toISOString(),
            },
        }) as any,
    getPublicProfile: async () =>
        ({
            ok: true,
            value: {
                id: "mock",
                username: "mock",
                isBanned: false,
                createdAt: new Date().toISOString(),
            },
        }) as any,
    upsertFromDiscord: async () => ({ ok: true, value: {} }) as any,
    updateLastSeen: async () => ({ ok: true, value: {} }) as any,
} as any;

/**
 * Mock Tap Service
 */
export const mockTapService: TapService = {
    listTaps: async () =>
        ({
            ok: true,
            value: {
                data: [],
                meta: { page: 1, perPage: 20, total: 0, totalPages: 0 },
            },
        }) as any,
    createTap: async () =>
        ({
            ok: true,
            value: {
                id: "mock",
                name: "Mock",
                ownerId: "mock",
                isPrivate: false,
                isLocked: false,
                isVerified: false,
                memberCount: 0,
                createdAt: new Date().toISOString(),
                updatedAt: new Date().toISOString(),
            },
        }) as any,
    getTap: async () =>
        ({
            ok: true,
            value: {
                id: "mock",
                name: "Mock",
                ownerId: "mock",
                isPrivate: false,
                isLocked: false,
                isVerified: false,
                memberCount: 0,
                createdAt: new Date().toISOString(),
                updatedAt: new Date().toISOString(),
            },
        }) as any,
    updateTap: async () =>
        ({
            ok: true,
            value: {
                id: "mock",
                name: "Mock",
                ownerId: "mock",
                isPrivate: false,
                isLocked: false,
                isVerified: false,
                memberCount: 0,
                createdAt: new Date().toISOString(),
                updatedAt: new Date().toISOString(),
            },
        }) as any,
    deleteTap: async () => ({ ok: true, value: undefined }) as any,
    getTapMembers: async () => ({ ok: true, value: [] }) as any,
    addMember: async () => ({ ok: true, value: {} }) as any,
    removeMember: async () => ({ ok: true, value: undefined }) as any,
    updateMemberRole: async () => ({ ok: true, value: {} }) as any,
} as any;
