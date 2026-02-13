import type { ResolvedValue } from '../resolver';
import type { ISettingsCache } from './index';

/**
 * Configuration for the memory cache.
 */
export interface MemoryCacheConfig {
  /** Default TTL in milliseconds */
  readonly defaultTtlMs: number;
}

/**
 * In-memory implementation of the settings cache.
 */
export class MemorySettingsCache implements ISettingsCache {
  private readonly cache = new Map<string, { value: unknown; expires: number }>();
  private readonly defaultTtlMs: number;

  constructor(config: MemoryCacheConfig) {
    this.defaultTtlMs = config.defaultTtlMs;
  }

  private getFullKey(keyId: string, contextKey: string): string {
    return `${keyId}:${contextKey}`;
  }

  async get<T>(keyId: string, contextKey: string): Promise<ResolvedValue<T> | undefined> {
    const fullKey = this.getFullKey(keyId, contextKey);
    const cached = this.cache.get(fullKey);

    if (cached && cached.expires > Date.now()) {
      return cached.value as ResolvedValue<T>;
    }

    if (cached) {
      this.cache.delete(fullKey);
    }

    return undefined;
  }

  async set<T>(
    keyId: string,
    contextKey: string,
    value: ResolvedValue<T>,
    ttlMs?: number
  ): Promise<void> {
    const fullKey = this.getFullKey(keyId, contextKey);
    this.cache.set(fullKey, {
      value,
      expires: Date.now() + (ttlMs ?? this.defaultTtlMs),
    });
  }

  async invalidate(keyId: string): Promise<void> {
    const prefix = `${keyId}:`;
    for (const cacheKey of Array.from(this.cache.keys())) {
      if (cacheKey.startsWith(prefix)) {
        this.cache.delete(cacheKey);
      }
    }
  }

  async clear(): Promise<void> {
    this.cache.clear();
  }
}

/**
 * Creates a memory settings cache.
 */
export function createMemoryCache(config: MemoryCacheConfig): ISettingsCache {
  return new MemorySettingsCache(config);
}
