import Fastify, { type FastifyInstance } from 'fastify';
import fastifySwagger from '@fastify/swagger';
import { SignJWT, exportJWK, generateKeyPair, jwtVerify, type KeyLike } from 'jose';
import { migrate } from 'drizzle-orm/postgres-js/migrator';
import { sql } from 'drizzle-orm';
import {
  UserId,
  GuildId,
  type AnySettingsKeyDefinition,
  ALL_USER_KEYS,
  ALL_GUILD_KEYS,
  ALL_ADMIN_KEYS,
} from 'zako3-settings';
import { createDatabase, createRedis, createLogger, type Database, type RedisClient } from '../../src/infra/index.js';
import {
  createSettingsService,
  createSettingsOperations,
  type SettingsService,
} from '../../src/service/index.js';
import { registerRoutes } from '../../src/routes/index.js';
import {
  registerAuthMiddleware,
  type IJWTVerifier,
  type IGuildPermissionChecker,
  type IBotAdminChecker,
} from '../../src/auth/index.js';
import type { Logger } from 'pino';

export interface TestContext {
  app: FastifyInstance;
  database: Database;
  redis: RedisClient;
  settings: SettingsService;
  logger: Logger;
  generateToken: (userId: string, options?: TokenOptions) => Promise<string>;
  cleanup: () => Promise<void>;
}

export interface TokenOptions {
  expiresIn?: string;
}

export interface MockPermissions {
  botAdmins: Set<string>;
  guildAdmins: Map<string, Set<string>>;
}

let privateKey: KeyLike | undefined;
let publicKey: KeyLike | undefined;
let keysInitialized = false;
let sharedDatabase: Database | undefined;
let sharedRedis: RedisClient | undefined;
let sharedLogger: Logger | undefined;
let migrationsComplete = false;
let migrationsPromise: Promise<void> | undefined;

async function initializeKeys(): Promise<void> {
  if (keysInitialized) return;
  
  const keyPair = await generateKeyPair('RS256');
  privateKey = keyPair.privateKey;
  publicKey = keyPair.publicKey;
  keysInitialized = true;
}

async function initializeSharedResources(): Promise<void> {
  if (sharedDatabase) return;

  sharedLogger = createLogger({
    serviceName: 'zako3-hq-test',
    serviceVersion: '0.0.0-test',
    environment: 'test',
    level: 'error',
  });

  sharedDatabase = createDatabase(
    { url: process.env.DATABASE_URL! },
    sharedLogger
  );
  
  sharedRedis = createRedis({ url: process.env.REDIS_URL! }, sharedLogger);
}

async function runMigrations(): Promise<void> {
  // If migrations are already complete, return immediately
  if (migrationsComplete) return;

  // If another call is already running migrations, wait for it
  if (migrationsPromise) {
    await migrationsPromise;
    return;
  }

  if (!sharedDatabase) throw new Error('Database not initialized');

  // Start migration - set the promise so other calls can wait
  migrationsPromise = (async () => {
    await migrate(sharedDatabase!.db, {
      migrationsFolder: './src/db/migrations',
    });
    migrationsComplete = true;
  })();

  await migrationsPromise;
}

function createMockJWTVerifier(): IJWTVerifier {
  return {
    async verify(token: string) {
      if (!publicKey) {
        return { ok: false as const, error: 'Keys not initialized' };
      }
      try {
        const result = await jwtVerify(token, publicKey);
        const payload = result.payload;

        if (!payload.sub || typeof payload.sub !== 'string') {
          return { ok: false as const, error: 'Invalid token' };
        }

        return {
          ok: true as const,
          value: {
            sub: payload.sub,
            iat: payload.iat ?? Math.floor(Date.now() / 1000),
            exp: payload.exp ?? Math.floor(Date.now() / 1000) + 3600,
          },
        };
      } catch {
        return { ok: false as const, error: 'Invalid token' };
      }
    },
  };
}

function createMockGuildChecker(permissions: MockPermissions): IGuildPermissionChecker {
  return {
    async isGuildAdmin(userId: UserId, guildId: GuildId): Promise<boolean> {
      const guildAdmins = permissions.guildAdmins.get(guildId as string);
      return guildAdmins?.has(userId as string) ?? false;
    },
  };
}

function createMockAdminChecker(permissions: MockPermissions): IBotAdminChecker {
  return {
    async isBotAdmin(userId: UserId): Promise<boolean> {
      return permissions.botAdmins.has(userId as string);
    },
  };
}

export async function createTestContext(
  permissions: MockPermissions = { botAdmins: new Set(), guildAdmins: new Map() }
): Promise<TestContext> {
  await initializeKeys();
  await initializeSharedResources();
  await runMigrations();

  const database = sharedDatabase!;
  const redis = sharedRedis!;
  const logger = sharedLogger!;

  const settings = createSettingsService({
    database,
    redis,
    logger,
    keys: [...ALL_USER_KEYS, ...ALL_GUILD_KEYS, ...ALL_ADMIN_KEYS] as AnySettingsKeyDefinition[],
    cacheTtlMs: 1000,
  });

  const initResult = await settings.initialize();
  if (!initResult.ok) {
    throw new Error(`Failed to initialize settings: ${initResult.error}`);
  }

  const guildChecker = createMockGuildChecker(permissions);
  const adminChecker = createMockAdminChecker(permissions);

  const operations = createSettingsOperations({
    settingsService: settings,
    guildChecker,
    adminChecker,
  });

  const jwtVerifier = createMockJWTVerifier();

  const app = Fastify({ logger: false });

  // Register auth middleware directly on app (not as a plugin)
  // This ensures the hook propagates to all child contexts
  registerAuthMiddleware(app, {
    jwtVerifier,
    excludePaths: ['/healthz', '/docs'],
  });

  await app.register(fastifySwagger, {
    openapi: {
      info: {
        title: 'Test API',
        version: '0.0.0',
      },
      components: {
        securitySchemes: {
          bearerAuth: {
            type: 'http',
            scheme: 'bearer',
            bearerFormat: 'JWT',
          },
        },
      },
    },
  });

  await app.register(
    registerRoutes({
      database,
      redis,
      settings,
      operations,
    })
  );

  await app.ready();

  async function generateToken(
    userId: string,
    options: TokenOptions = {}
  ): Promise<string> {
    if (!privateKey) throw new Error('Keys not initialized');
    const { expiresIn = '1h' } = options;
    return new SignJWT({ sub: userId })
      .setProtectedHeader({ alg: 'RS256', kid: 'test-key-1' })
      .setIssuedAt()
      .setExpirationTime(expiresIn)
      .sign(privateKey);
  }

  async function cleanup(): Promise<void> {
    await database.db.execute(sql`TRUNCATE TABLE settings_entries`);
    await redis.client.flushdb();
  }

  return {
    app,
    database,
    redis,
    settings,
    logger,
    generateToken,
    cleanup,
  };
}

export async function destroyTestContext(ctx: TestContext): Promise<void> {
  await ctx.app.close();
  await ctx.settings.shutdown();
}
