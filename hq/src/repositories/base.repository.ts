import type { Redis } from 'ioredis';
import type { Logger } from 'pino';
import type { Database } from '../infra/database.js';

/**
 * Base repository class with common database and caching functionality
 */
export abstract class BaseRepository {
  protected db: Database['db'];
  protected sql: Database['sql'];
  protected redis: Redis;
  protected logger: Logger;
  protected cachePrefix: string;

  constructor(
    database: Database,
    redis: Redis,
    logger: Logger,
    cachePrefix: string,
  ) {
    this.db = database.db;
    this.sql = database.sql;
    this.redis = redis;
    this.logger = logger.child({ repository: this.constructor.name });
    this.cachePrefix = cachePrefix;
  }

  /**
   * Get a value from cache
   */
  protected async getCache<T>(key: string): Promise<T | null> {
    try {
      const data = await this.redis.get(this.getCacheKey(key));
      if (!data) return null;
      return JSON.parse(data) as T;
    } catch (error) {
      this.logger.warn({ error, key }, 'Cache get failed');
      return null;
    }
  }

  /**
   * Set a value in cache with TTL (in seconds)
   */
  protected async setCache<T>(
    key: string,
    value: T,
    ttl: number = 300, // 5 minutes default
  ): Promise<void> {
    try {
      await this.redis.setex(
        this.getCacheKey(key),
        ttl,
        JSON.stringify(value),
      );
    } catch (error) {
      this.logger.warn({ error, key }, 'Cache set failed');
    }
  }

  /**
   * Delete a value from cache
   */
  protected async deleteCache(key: string): Promise<void> {
    try {
      await this.redis.del(this.getCacheKey(key));
    } catch (error) {
      this.logger.warn({ error, key }, 'Cache delete failed');
    }
  }

  /**
   * Delete multiple cache keys by pattern
   */
  protected async deleteCachePattern(pattern: string): Promise<void> {
    try {
      const keys = await this.redis.keys(this.getCacheKey(pattern));
      if (keys.length > 0) {
        await this.redis.del(...keys);
      }
    } catch (error) {
      this.logger.warn({ error, pattern }, 'Cache pattern delete failed');
    }
  }

  /**
   * Get full cache key with prefix
   */
  protected getCacheKey(key: string): string {
    return `${this.cachePrefix}:${key}`;
  }

  /**
   * Execute a function with cache-aside pattern
   * @param key Cache key
   * @param fn Function to execute if cache misses
   * @param ttl Cache TTL in seconds
   */
  protected async cacheAside<T>(
    key: string,
    fn: () => Promise<T>,
    ttl: number = 300,
  ): Promise<T> {
    // Try to get from cache
    const cached = await this.getCache<T>(key);
    if (cached !== null) {
      this.logger.debug({ key }, 'Cache hit');
      return cached;
    }

    this.logger.debug({ key }, 'Cache miss');
    
    // Execute function
    const result = await fn();

    // Store in cache
    await this.setCache(key, result, ttl);

    return result;
  }
}
