import type { Redis } from 'ioredis';
import type { ISettingsCache, ResolvedValue } from 'zako3-settings';
import type { Logger } from 'pino';

export interface RedisCacheConfig {
  client: Redis;
  logger: Logger;
  defaultTtlMs?: number;
  keyPrefix?: string;
}

export function createRedisCache(config: RedisCacheConfig): ISettingsCache {
  const {
    client,
    logger,
    defaultTtlMs = 60000,
    keyPrefix = 'settings:',
  } = config;
  const log = logger.child({ module: 'settings-cache' });

  function makeKey(keyId: string, contextKey: string): string {
    return `${keyPrefix}${keyId}:${contextKey}`;
  }

  function makePatternPrefix(keyId: string): string {
    return `${keyPrefix}${keyId}:`;
  }

  return {
    async get<T>(
      keyId: string,
      contextKey: string
    ): Promise<ResolvedValue<T> | undefined> {
      const key = makeKey(keyId, contextKey);
      try {
        const data = await client.get(key);
        if (data === null) {
          return undefined;
        }
        return JSON.parse(data) as ResolvedValue<T>;
      } catch (err) {
        log.warn({ err, key }, 'Cache get failed');
        return undefined;
      }
    },

    async set<T>(
      keyId: string,
      contextKey: string,
      value: ResolvedValue<T>,
      ttlMs?: number
    ): Promise<void> {
      const key = makeKey(keyId, contextKey);
      const ttl = ttlMs ?? defaultTtlMs;
      try {
        await client.set(key, JSON.stringify(value), 'PX', ttl);
      } catch (err) {
        log.warn({ err, key }, 'Cache set failed');
      }
    },

    async invalidate(keyId: string): Promise<void> {
      const patternPrefix = makePatternPrefix(keyId);
      const clientPrefix = (client.options.keyPrefix as string) ?? '';
      const fullPattern = `${clientPrefix}${patternPrefix}*`;

      try {
        const stream = client.scanStream({
          match: fullPattern,
          count: 100,
        });

        const keysToDelete: string[] = [];

        for await (const keys of stream) {
          for (const key of keys as string[]) {
            const keyWithoutPrefix = clientPrefix
              ? key.slice(clientPrefix.length)
              : key;
            keysToDelete.push(keyWithoutPrefix);
          }
        }

        if (keysToDelete.length > 0) {
          await client.del(...keysToDelete);
          log.debug(
            { keyId, count: keysToDelete.length },
            'Cache entries invalidated'
          );
        }
      } catch (err) {
        log.warn({ err, keyId }, 'Cache invalidation failed');
      }
    },

    async clear(): Promise<void> {
      const clientPrefix = (client.options.keyPrefix as string) ?? '';
      const fullPattern = `${clientPrefix}${keyPrefix}*`;

      try {
        const stream = client.scanStream({
          match: fullPattern,
          count: 100,
        });

        const keysToDelete: string[] = [];

        for await (const keys of stream) {
          for (const key of keys as string[]) {
            const keyWithoutPrefix = clientPrefix
              ? key.slice(clientPrefix.length)
              : key;
            keysToDelete.push(keyWithoutPrefix);
          }
        }

        if (keysToDelete.length > 0) {
          await client.del(...keysToDelete);
          log.info({ count: keysToDelete.length }, 'Cache cleared');
        }
      } catch (err) {
        log.warn({ err }, 'Cache clear failed');
      }
    },
  };
}
