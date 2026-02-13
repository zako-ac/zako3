import type { ResolvedValue } from '../resolver';

/**
 * Interface for settings cache.
 */
export interface ISettingsCache {
  /**
   * Gets a cached value.
   * 
   * @param keyId - The setting identifier
   * @param contextKey - A string representing the resolution context
   */
  get<T>(keyId: string, contextKey: string): Promise<ResolvedValue<T> | undefined>;

  /**
   * Sets a cached value.
   * 
   * @param keyId - The setting identifier
   * @param contextKey - A string representing the resolution context
   * @param value - The value to cache
   * @param ttlMs - Optional TTL override
   */
  set<T>(
    keyId: string,
    contextKey: string,
    value: ResolvedValue<T>,
    ttlMs?: number
  ): Promise<void>;

  /**
   * Invalidates all cache entries for a specific key.
   * 
   * @param keyId - The setting identifier
   */
  invalidate(keyId: string): Promise<void>;

  /**
   * Clears the entire cache.
   */
  clear(): Promise<void>;
}
