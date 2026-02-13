/**
 * @fileoverview Persistence adapter interface for dependency injection.
 *
 * This module defines the interface that persistence implementations must
 * satisfy. The actual storage implementation (database, file, memory, etc.)
 * is injected at runtime.
 */

import type { KeyIdentifier, Result } from '../types';
import type { Scope, SettingsKind } from '../scope';
import type { StoredEntry } from '../entry';

// =============================================================================
// Query Types
// =============================================================================

/**
 * Query for fetching entries.
 */
export interface EntryQuery {
  /** The key identifier to query */
  readonly keyIdentifier: KeyIdentifier;

  /** Optional: filter by scopes */
  readonly scopes?: readonly Scope[];

  /** Optional: filter by settings kind */
  readonly settingsKind?: SettingsKind;
}

/**
 * Query for fetching multiple entries by scope pattern.
 */
export interface ScopeQuery {
  /** The settings kind */
  readonly settingsKind: SettingsKind;

  /** Optional: guild ID filter */
  readonly guildId?: string;

  /** Optional: user ID filter */
  readonly userId?: string;
}

/**
 * Batch query for multiple keys.
 */
export interface BatchEntryQuery {
  /** The key identifiers to query */
  readonly keyIdentifiers: readonly KeyIdentifier[];

  /** The scopes to query */
  readonly scopes: readonly Scope[];
}

// =============================================================================
// Persistence Adapter Interface
// =============================================================================

/**
 * Interface for persistence implementations.
 *
 * Implementations of this interface handle the actual storage and retrieval
 * of settings entries. This could be backed by:
 * - A database (PostgreSQL, MongoDB, etc.)
 * - A file system
 * - An in-memory store (for testing)
 * - A remote API
 *
 * All methods are async to support various backend types.
 */
export interface IPersistenceAdapter {
  // ===========================================================================
  // Read Operations
  // ===========================================================================

  /**
   * Fetches a single entry by key and exact scope.
   *
   * @param keyIdentifier - The key to fetch
   * @param scope - The exact scope to fetch from
   * @returns The entry if found, or undefined
   */
  getEntry(
    keyIdentifier: KeyIdentifier,
    scope: Scope
  ): Promise<StoredEntry | undefined>;

  /**
   * Fetches all entries for a key across specified scopes.
   *
   * @param keyIdentifier - The key to fetch
   * @param scopes - The scopes to query
   * @returns Array of entries found
   */
  getEntriesForKey(
    keyIdentifier: KeyIdentifier,
    scopes: readonly Scope[]
  ): Promise<readonly StoredEntry[]>;

  /**
   * Fetches entries for multiple keys at once.
   * This is more efficient than multiple single-key queries.
   *
   * @param query - The batch query
   * @returns Map of key identifier to entries
   */
  getEntriesBatch(
    query: BatchEntryQuery
  ): Promise<ReadonlyMap<string, readonly StoredEntry[]>>;

  /**
   * Fetches all entries matching a scope pattern.
   *
   * @param query - The scope query
   * @returns Array of entries found
   */
  getEntriesByScope(query: ScopeQuery): Promise<readonly StoredEntry[]>;

  /**
   * Checks if an entry exists.
   *
   * @param keyIdentifier - The key to check
   * @param scope - The scope to check
   * @returns True if the entry exists
   */
  hasEntry(keyIdentifier: KeyIdentifier, scope: Scope): Promise<boolean>;

  // ===========================================================================
  // Write Operations
  // ===========================================================================

  /**
   * Sets (creates or updates) an entry.
   *
   * @param entry - The entry to store
   * @returns Result indicating success or failure
   */
  setEntry(entry: StoredEntry): Promise<Result<void>>;

  /**
   * Sets multiple entries atomically.
   *
   * @param entries - The entries to store
   * @returns Result indicating success or failure
   */
  setEntriesBatch(entries: readonly StoredEntry[]): Promise<Result<void>>;

  /**
   * Deletes an entry.
   *
   * @param keyIdentifier - The key to delete
   * @param scope - The scope to delete from
   * @returns True if an entry was deleted, false if it didn't exist
   */
  deleteEntry(keyIdentifier: KeyIdentifier, scope: Scope): Promise<boolean>;

  /**
   * Deletes all entries for a key.
   *
   * @param keyIdentifier - The key to delete all entries for
   * @returns The number of entries deleted
   */
  deleteEntriesForKey(keyIdentifier: KeyIdentifier): Promise<number>;

  /**
   * Deletes all entries matching a scope pattern.
   *
   * @param query - The scope query
   * @returns The number of entries deleted
   */
  deleteEntriesByScope(query: ScopeQuery): Promise<number>;

  // ===========================================================================
  // Utility Operations
  // ===========================================================================

  /**
   * Counts entries matching a query.
   *
   * @param query - The entry query
   * @returns The count of matching entries
   */
  countEntries(query: EntryQuery): Promise<number>;

  /**
   * Lists all unique key identifiers that have entries.
   *
   * @param settingsKind - Optional filter by settings kind
   * @returns Array of key identifiers
   */
  listKeys(settingsKind?: SettingsKind): Promise<readonly string[]>;

  // ===========================================================================
  // Lifecycle Operations
  // ===========================================================================

  /**
   * Initializes the persistence adapter.
   * Called once when the settings manager starts.
   *
   * @returns Result indicating success or failure
   */
  initialize(): Promise<Result<void>>;

  /**
   * Closes the persistence adapter.
   * Called when the settings manager shuts down.
   */
  close(): Promise<void>;

  /**
   * Checks if the adapter is healthy and connected.
   *
   * @returns True if healthy
   */
  isHealthy(): Promise<boolean>;
}

// =============================================================================
// Adapter Factory Type
// =============================================================================

/**
 * Factory function type for creating persistence adapters.
 */
export type PersistenceAdapterFactory<TConfig = unknown> = (
  config: TConfig
) => IPersistenceAdapter;

// =============================================================================
// In-Memory Adapter (for testing)
// =============================================================================

/**
 * Creates an in-memory persistence adapter.
 * Useful for testing and development.
 *
 * @returns An in-memory persistence adapter
 */
export function createInMemoryAdapter(): IPersistenceAdapter {
  const store = new Map<string, StoredEntry>();

  function makeKey(keyIdentifier: string, scope: Scope): string {
    const scopeKey = JSON.stringify(scope);
    return `${keyIdentifier}::${scopeKey}`;
  }

  function matchesScope(entry: StoredEntry, query: ScopeQuery): boolean {
    if (entry.scope.kind !== query.settingsKind) {
      return false;
    }
    if (query.guildId && entry.scope.guildId !== query.guildId) {
      return false;
    }
    if (query.userId && entry.scope.userId !== query.userId) {
      return false;
    }
    return true;
  }

  return {
    async getEntry(keyIdentifier, scope) {
      const key = makeKey(keyIdentifier as string, scope);
      return store.get(key);
    },

    async getEntriesForKey(keyIdentifier, scopes) {
      const entries: StoredEntry[] = [];
      for (const scope of scopes) {
        const key = makeKey(keyIdentifier as string, scope);
        const entry = store.get(key);
        if (entry) {
          entries.push(entry);
        }
      }
      return entries;
    },

    async getEntriesBatch(query) {
      const result = new Map<string, StoredEntry[]>();

      for (const keyId of query.keyIdentifiers) {
        const entries: StoredEntry[] = [];
        for (const scope of query.scopes) {
          const key = makeKey(keyId as string, scope);
          const entry = store.get(key);
          if (entry) {
            entries.push(entry);
          }
        }
        result.set(keyId as string, entries);
      }

      return result;
    },

    async getEntriesByScope(query) {
      const entries: StoredEntry[] = [];
      for (const entry of Array.from(store.values())) {
        if (matchesScope(entry, query)) {
          entries.push(entry);
        }
      }
      return entries;
    },

    async hasEntry(keyIdentifier, scope) {
      const key = makeKey(keyIdentifier as string, scope);
      return store.has(key);
    },

    async setEntry(entry) {
      const scope = {
        kind: entry.scope.kind,
        scope: entry.scope.scope,
        guildId: entry.scope.guildId,
        userId: entry.scope.userId,
      } as Scope;
      const key = makeKey(entry.keyIdentifier, scope);
      store.set(key, entry);
      return { ok: true, value: undefined };
    },

    async setEntriesBatch(entries) {
      for (const entry of entries) {
        const scope = {
          kind: entry.scope.kind,
          scope: entry.scope.scope,
          guildId: entry.scope.guildId,
          userId: entry.scope.userId,
        } as Scope;
        const key = makeKey(entry.keyIdentifier, scope);
        store.set(key, entry);
      }
      return { ok: true, value: undefined };
    },

    async deleteEntry(keyIdentifier, scope) {
      const key = makeKey(keyIdentifier as string, scope);
      return store.delete(key);
    },

    async deleteEntriesForKey(keyIdentifier) {
      let count = 0;
      const keyIdStr = keyIdentifier as string;
      for (const [key] of Array.from(store.entries())) {
        if (key.startsWith(`${keyIdStr}::`)) {
          store.delete(key);
          count++;
        }
      }
      return count;
    },

    async deleteEntriesByScope(query) {
      let count = 0;
      for (const [key, entry] of Array.from(store.entries())) {
        if (matchesScope(entry, query)) {
          store.delete(key);
          count++;
        }
      }
      return count;
    },

    async countEntries(query) {
      let count = 0;
      const keyIdStr = query.keyIdentifier as string;
      for (const entry of Array.from(store.values())) {
        if (entry.keyIdentifier === keyIdStr) {
          if (!query.settingsKind || entry.scope.kind === query.settingsKind) {
            count++;
          }
        }
      }
      return count;
    },

    async listKeys(settingsKind) {
      const keys = new Set<string>();
      for (const entry of Array.from(store.values())) {
        if (!settingsKind || entry.scope.kind === settingsKind) {
          keys.add(entry.keyIdentifier);
        }
      }
      return Array.from(keys);
    },

    async initialize() {
      return { ok: true, value: undefined };
    },

    async close() {
      store.clear();
    },

    async isHealthy() {
      return true;
    },
  };
}
