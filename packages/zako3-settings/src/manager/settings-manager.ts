/**
 * @fileoverview Main settings manager - the primary entry point for the settings system.
 *
 * The SettingsManager orchestrates all settings operations, including:
 * - Getting and setting values
 * - Permission checking
 * - Cache management (optional)
 * - Lifecycle management
 */

import type { KeyIdentifier, Result } from '../types';
import { ok, err } from '../types';
import type { Scope, SettingsActor } from '../scope';
import { canActorWriteToScope } from '../scope';
import type {
  AnySettingsKeyDefinition,
  UserSettingsKeyDefinition,
  KeyRegistry,
} from '../keys';
import {
  createKeyRegistry,
  isScopeAllowedForUserKey,
  isUserKey,
  serializeKeyValue,
  validateKeyValue,
} from '../keys';
import type { SettingsEntry, StoredEntry } from '../entry';
import { serializeScope } from '../entry';
import type { IPersistenceAdapter } from '../persistence';
import type { ResolutionContext, ResolvedValue, ISettingsResolver } from '../resolver';
import { createResolver } from '../resolver';
import type { ISettingsCache } from '../cache';
import { createMemoryCache } from '../cache/memory';

// =============================================================================
// Settings Manager Configuration
// =============================================================================

/**
 * Configuration for the settings manager.
 */
export interface SettingsManagerConfig {
  /** The persistence adapter for storage */
  readonly persistence: IPersistenceAdapter;

  /** Optional: Pre-registered key definitions */
  readonly keys?: readonly AnySettingsKeyDefinition[];

  /** Optional: Enable caching (default: false) */
  readonly enableCache?: boolean;

  /** Optional: Custom cache implementation. If not provided and enableCache is true, a memory cache is used. */
  readonly cache?: ISettingsCache;

  /** Optional: Cache TTL in milliseconds (default: 60000) */
  readonly cacheTtlMs?: number;
}

// =============================================================================
// Settings Manager Interface
// =============================================================================

/**
 * Main settings manager interface.
 */
export interface ISettingsManager {
  // ===========================================================================
  // Lifecycle
  // ===========================================================================

  /**
   * Initializes the settings manager.
   * Must be called before any other operations.
   */
  initialize(): Promise<Result<void>>;

  /**
   * Shuts down the settings manager.
   */
  shutdown(): Promise<void>;

  /**
   * Checks if the manager is initialized and healthy.
   */
  isHealthy(): Promise<boolean>;

  // ===========================================================================
  // Key Registry
  // ===========================================================================

  /**
   * Gets the key registry.
   */
  readonly registry: KeyRegistry;

  /**
   * Registers a key definition.
   */
  registerKey(key: AnySettingsKeyDefinition): void;

  /**
   * Registers multiple key definitions.
   */
  registerKeys(keys: readonly AnySettingsKeyDefinition[]): void;

  // ===========================================================================
  // Read Operations
  // ===========================================================================

  /**
   * Gets a setting value for a context.
   *
   * @param key - The key definition or identifier
   * @param context - The resolution context
   * @returns The resolved value
   */
  get<T>(
    key: AnySettingsKeyDefinition | KeyIdentifier,
    context: ResolutionContext
  ): Promise<Result<ResolvedValue<T>>>;

  /**
   * Gets multiple setting values at once.
   *
   * @param keys - The key definitions
   * @param context - The resolution context
   * @returns Map of key identifier to resolved value
   */
  getMany(
    keys: readonly AnySettingsKeyDefinition[],
    context: ResolutionContext
  ): Promise<Result<ReadonlyMap<string, ResolvedValue<unknown>>>>;

  /**
   * Gets the raw value at a specific scope (not resolved).
   *
   * @param key - The key definition or identifier
   * @param scope - The exact scope to read from
   * @returns The entry if found
   */
  getAtScope<T>(
    key: AnySettingsKeyDefinition | KeyIdentifier,
    scope: Scope
  ): Promise<Result<SettingsEntry<T, Scope> | undefined>>;

  // ===========================================================================
  // Write Operations
  // ===========================================================================

  /**
   * Sets a setting value.
   *
   * @param key - The key definition or identifier
   * @param value - The value to set
   * @param scope - The scope to set at
   * @param actor - The actor performing the action (for permission check)
   * @param isImportant - Whether this is an important entry
   * @returns Result indicating success or failure
   */
  set<T>(
    key: AnySettingsKeyDefinition | KeyIdentifier,
    value: T,
    scope: Scope,
    actor: SettingsActor,
    isImportant?: boolean
  ): Promise<Result<void>>;

  /**
   * Deletes a setting entry.
   *
   * @param key - The key definition or identifier
   * @param scope - The scope to delete from
   * @param actor - The actor performing the action
   * @returns True if an entry was deleted
   */
  delete(
    key: AnySettingsKeyDefinition | KeyIdentifier,
    scope: Scope,
    actor: SettingsActor
  ): Promise<Result<boolean>>;

  /**
   * Resets a setting to its default value by deleting all entries.
   *
   * @param key - The key definition or identifier
   * @param actor - The actor performing the action
   * @returns The number of entries deleted
   */
  reset(
    key: AnySettingsKeyDefinition | KeyIdentifier,
    actor: SettingsActor
  ): Promise<Result<number>>;

  // ===========================================================================
  // Cache Operations
  // ===========================================================================

  /**
   * Invalidates the cache for a specific key.
   */
  invalidateCache(keyIdentifier: KeyIdentifier): void;

  /**
   * Clears the entire cache.
   */
  clearCache(): void;

  // ===========================================================================
  // Internal Access
  // ===========================================================================

  /**
   * Gets the resolver instance.
   */
  readonly resolver: ISettingsResolver;

  /**
   * Gets the persistence adapter.
   */
  readonly persistence: IPersistenceAdapter;
}

// =============================================================================
// Settings Manager Implementation
// =============================================================================

/**
 * Creates a settings manager.
 *
 * @param config - Manager configuration
 * @returns A settings manager
 */
export function createSettingsManager(config: SettingsManagerConfig): ISettingsManager {
  const { persistence, enableCache = false, cacheTtlMs = 60000 } = config;

  // Internal state
  const registry = createKeyRegistry();
  const resolver = createResolver(persistence);
  let initialized = false;

  // Cache setup
  let cache: ISettingsCache | null = null;
  if (config.cache) {
    cache = config.cache;
  } else if (enableCache) {
    cache = createMemoryCache({ defaultTtlMs: cacheTtlMs });
  }

  /**
   * Resolves a key from identifier or definition.
   */
  function resolveKey(key: AnySettingsKeyDefinition | KeyIdentifier): AnySettingsKeyDefinition {
    if (typeof key === 'object' && 'identifier' in key) {
      return key;
    }
    const found = registry.get(key);
    if (!found) {
      throw new Error(`Unknown settings key: ${key}`);
    }
    return found;
  }

  /**
   * Gets the cache context key for a resolution.
   */
  function getCacheContextKey(context: ResolutionContext): string {
    return JSON.stringify(context);
  }

  /**
   * Checks if actor can write to key/scope combination.
   */
  function checkWritePermission(
    key: AnySettingsKeyDefinition,
    scope: Scope,
    actor: SettingsActor
  ): Result<void> {
    // Admin-only keys require admin actor
    if (key.adminOnly && actor.kind !== 'admin') {
      return err(`Key '${key.identifier}' is admin-only`);
    }

    // Check if actor can write to scope
    if (!canActorWriteToScope(actor, scope)) {
      return err(`Actor cannot write to scope: ${JSON.stringify(scope)}`);
    }

    // For user keys, check if scope is allowed
    if (isUserKey(key)) {
      const userKey = key as UserSettingsKeyDefinition;
      if (!isScopeAllowedForUserKey(userKey, scope.scope as any)) {
        return err(`Scope '${scope.scope}' is not allowed for key '${key.identifier}'`);
      }
    }

    return ok(undefined);
  }

  return {
    // =========================================================================
    // Lifecycle
    // =========================================================================

    async initialize(): Promise<Result<void>> {
      if (initialized) {
        return ok(undefined);
      }

      const result = await persistence.initialize();
      if (!result.ok) {
        return result;
      }

      // Register provided keys
      if (config.keys) {
        registry.registerMany(config.keys);
      }

      initialized = true;
      return ok(undefined);
    },

    async shutdown(): Promise<void> {
      if (cache) {
        cache.clear();
      }
      await persistence.close();
      initialized = false;
    },

    async isHealthy(): Promise<boolean> {
      if (!initialized) {
        return false;
      }
      return persistence.isHealthy();
    },

    // =========================================================================
    // Key Registry
    // =========================================================================

    registry,

    registerKey(key: AnySettingsKeyDefinition): void {
      registry.register(key);
    },

    registerKeys(keys: readonly AnySettingsKeyDefinition[]): void {
      registry.registerMany(keys);
    },

    // =========================================================================
    // Read Operations
    // =========================================================================

    async get<T>(
      key: AnySettingsKeyDefinition | KeyIdentifier,
      context: ResolutionContext
    ): Promise<Result<ResolvedValue<T>>> {
      if (!initialized) {
        return err('Settings manager not initialized');
      }

      try {
        const keyDef = resolveKey(key);

        // Check cache
        if (cache) {
          const contextKey = getCacheContextKey(context);
          const cachedValue = await cache.get<T>(keyDef.identifier as string, contextKey);
          if (cachedValue !== undefined) {
            return ok(cachedValue);
          }
        }

        // Resolve
        const result = await resolver.resolve<T>(keyDef, context);

        // Cache result
        if (result.ok && cache) {
          const contextKey = getCacheContextKey(context);
          await cache.set(keyDef.identifier as string, contextKey, result.value);
        }

        return result;
      } catch (error) {
        return err(`Failed to get setting: ${error}`);
      }
    },

    async getMany(
      keys: readonly AnySettingsKeyDefinition[],
      context: ResolutionContext
    ): Promise<Result<ReadonlyMap<string, ResolvedValue<unknown>>>> {
      if (!initialized) {
        return err('Settings manager not initialized');
      }

      return resolver.resolveMany(keys, context);
    },

    async getAtScope<T>(
      key: AnySettingsKeyDefinition | KeyIdentifier,
      scope: Scope
    ): Promise<Result<SettingsEntry<T, Scope> | undefined>> {
      if (!initialized) {
        return err('Settings manager not initialized');
      }

      try {
        const keyDef = resolveKey(key);
        const stored = await persistence.getEntry(keyDef.identifier, scope);

        if (!stored) {
          return ok(undefined);
        }

        // Deserialize value
        const valueResult = keyDef.valueType.deserialize(stored.value);
        if (!valueResult.ok) {
          return err(`Failed to deserialize value: ${valueResult.error}`);
        }

        return ok({
          keyIdentifier: keyDef.identifier,
          value: valueResult.value as T,
          scope,
          isImportant: stored.isImportant,
        });
      } catch (error) {
        return err(`Failed to get setting at scope: ${error}`);
      }
    },

    // =========================================================================
    // Write Operations
    // =========================================================================

    async set<T>(
      key: AnySettingsKeyDefinition | KeyIdentifier,
      value: T,
      scope: Scope,
      actor: SettingsActor,
      isImportant: boolean = false
    ): Promise<Result<void>> {
      if (!initialized) {
        return err('Settings manager not initialized');
      }

      try {
        const keyDef = resolveKey(key);

        // Check permissions
        const permResult = checkWritePermission(keyDef, scope, actor);
        if (!permResult.ok) {
          return permResult;
        }

        // Validate value
        const validationResult = validateKeyValue(keyDef, value);
        if (!validationResult.ok) {
          return err(`Invalid value: ${validationResult.error}`);
        }

        // Serialize and store
        const stored: StoredEntry = {
          keyIdentifier: keyDef.identifier as string,
          value: serializeKeyValue(keyDef, value),
          scope: serializeScope(scope),
          isImportant,
        };

        const result = await persistence.setEntry(stored);

        // Invalidate cache
        if (result.ok && cache) {
          await cache.invalidate(keyDef.identifier as string);
        }

        return result;
      } catch (error) {
        return err(`Failed to set setting: ${error}`);
      }
    },

    async delete(
      key: AnySettingsKeyDefinition | KeyIdentifier,
      scope: Scope,
      actor: SettingsActor
    ): Promise<Result<boolean>> {
      if (!initialized) {
        return err('Settings manager not initialized');
      }

      try {
        const keyDef = resolveKey(key);

        // Check permissions
        const permResult = checkWritePermission(keyDef, scope, actor);
        if (!permResult.ok) {
          return err(permResult.error);
        }

        const deleted = await persistence.deleteEntry(keyDef.identifier, scope);

        // Invalidate cache
        if (deleted && cache) {
          await cache.invalidate(keyDef.identifier as string);
        }

        return ok(deleted);
      } catch (error) {
        return err(`Failed to delete setting: ${error}`);
      }
    },

    async reset(
      key: AnySettingsKeyDefinition | KeyIdentifier,
      actor: SettingsActor
    ): Promise<Result<number>> {
      if (!initialized) {
        return err('Settings manager not initialized');
      }

      // Only admins can reset all entries
      if (actor.kind !== 'admin') {
        return err('Only admins can reset all entries for a key');
      }

      try {
        const keyDef = resolveKey(key);
        const count = await persistence.deleteEntriesForKey(keyDef.identifier);

        // Clear cache for this key
        if (cache) {
          await cache.invalidate(keyDef.identifier as string);
        }

        return ok(count);
      } catch (error) {
        return err(`Failed to reset setting: ${error}`);
      }
    },

    // =========================================================================
    // Cache Operations
    // =========================================================================

    invalidateCache(keyIdentifier: KeyIdentifier): void {
      if (!cache) return;

      // Note: With the new cache interface, we invalidate by key identifier.
      // Context-specific invalidation (by scope) is handled by invalidating the whole key
      // for simplicity and to match the previous behavior's effectiveness.
      cache.invalidate(keyIdentifier as string).catch(() => {
        // Log or handle error if needed
      });
    },

    clearCache(): void {
      if (cache) {
        cache.clear().catch(() => {
          // Log or handle error if needed
        });
      }
    },

    // =========================================================================
    // Internal Access
    // =========================================================================

    resolver,
    persistence,
  };
}
