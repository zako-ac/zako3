import { Redis } from 'ioredis';
import type { Logger } from 'pino';

export interface RedisConfig {
  url: string;
  keyPrefix?: string;
  maxRetriesPerRequest?: number;
  lazyConnect?: boolean;
}

export interface RedisClient {
  client: Redis;
  close: () => Promise<void>;
  isHealthy: () => Promise<boolean>;
}

export function createRedis(config: RedisConfig, logger: Logger): RedisClient {
  const log = logger.child({ module: 'redis' });

  const client = new Redis(config.url, {
    keyPrefix: config.keyPrefix ?? 'zako3:',
    maxRetriesPerRequest: config.maxRetriesPerRequest ?? 3,
    lazyConnect: config.lazyConnect ?? false,
  });

  client.on('connect', () => log.info('Redis connected'));
  client.on('error', (err: Error) => log.error({ err }, 'Redis error'));
  client.on('close', () => log.warn('Redis connection closed'));
  client.on('reconnecting', () => log.info('Redis reconnecting'));

  return {
    client,
    async close() {
      log.info('Closing Redis connection');
      await client.quit();
    },
    async isHealthy() {
      try {
        const pong = await client.ping();
        return pong === 'PONG';
      } catch (error) {
        log.warn({ error }, 'Redis health check failed');
        return false;
      }
    },
  };
}
