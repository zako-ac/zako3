import {
  createSettingsManager,
  type ISettingsManager,
  type AnySettingsKeyDefinition,
  type ResolutionContext,
  type SettingsActor,
  type Scope,
  type ResolvedValue,
  type Result,
  ALL_USER_KEYS,
  ALL_GUILD_KEYS,
  ALL_ADMIN_KEYS,
} from 'zako3-settings';
import type { Database } from '../../infra/database.js';
import type { RedisClient } from '../../infra/redis.js';
import type { Logger } from 'pino';
import { createDrizzleAdapter } from './adapter.js';
import { createRedisCache } from './cache.js';

export interface SettingsServiceConfig {
  database: Database;
  redis: RedisClient;
  logger: Logger;
  keys?: readonly AnySettingsKeyDefinition[];
  cacheTtlMs?: number;
}

export interface SettingsService {
  get<T>(
    key: AnySettingsKeyDefinition,
    context: ResolutionContext
  ): Promise<Result<ResolvedValue<T>>>;

  set<T>(
    key: AnySettingsKeyDefinition,
    value: T,
    scope: Scope,
    actor: SettingsActor
  ): Promise<Result<void>>;

  delete(
    key: AnySettingsKeyDefinition,
    scope: Scope,
    actor: SettingsActor
  ): Promise<Result<boolean>>;

  initialize(): Promise<Result<void>>;

  shutdown(): Promise<void>;

  isHealthy(): Promise<boolean>;

  readonly manager: ISettingsManager;
}

export function createSettingsService(config: SettingsServiceConfig): SettingsService {
  const { database, redis, logger, cacheTtlMs = 60000 } = config;
  const log = logger.child({ module: 'settings-service' });

  const keys = config.keys ?? [...ALL_USER_KEYS, ...ALL_GUILD_KEYS, ...ALL_ADMIN_KEYS];

  const adapter = createDrizzleAdapter({
    db: database.db,
    logger: log,
  });

  const cache = createRedisCache({
    client: redis.client,
    logger: log,
    defaultTtlMs: cacheTtlMs,
  });

  const manager = createSettingsManager({
    persistence: adapter,
    keys,
    enableCache: true,
    cache,
    cacheTtlMs,
  });

  return {
    get<T>(
      key: AnySettingsKeyDefinition,
      context: ResolutionContext
    ): Promise<Result<ResolvedValue<T>>> {
      return manager.get(key, context);
    },

    set<T>(
      key: AnySettingsKeyDefinition,
      value: T,
      scope: Scope,
      actor: SettingsActor
    ): Promise<Result<void>> {
      return manager.set(key, value, scope, actor);
    },

    delete(
      key: AnySettingsKeyDefinition,
      scope: Scope,
      actor: SettingsActor
    ): Promise<Result<boolean>> {
      return manager.delete(key, scope, actor);
    },

    async initialize(): Promise<Result<void>> {
      log.info('Initializing settings service');
      return manager.initialize();
    },

    async shutdown(): Promise<void> {
      log.info('Shutting down settings service');
      await manager.shutdown();
    },

    async isHealthy(): Promise<boolean> {
      return manager.isHealthy();
    },

    manager,
  };
}
