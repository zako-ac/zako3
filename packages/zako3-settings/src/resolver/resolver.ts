/**
 * @fileoverview Settings resolver for resolving final values from entries.
 *
 * The resolver handles:
 * - Building scope chains for resolution
 * - Fetching entries from persistence
 * - Applying precedence rules (including important entries)
 * - Merging values for precedence-merged keys
 */

import type { KeyIdentifier, Result } from '../types';
import { ok, err } from '../types';
import type { Scope, SettingsKind } from '../scope';
import { buildScopeChain } from '../scope';
import { findWinningEntry, filterByImportance } from '../scope';
import type { AnySettingsKeyDefinition } from '../keys';
import type { SettingsEntry } from '../entry';
import { deserializeEntry, createEntry, mergeEntriesWithDefault, isMergeableType } from '../entry';
import type { IPersistenceAdapter } from '../persistence';
import type { ResolutionContext } from './context';
import { contextToScopeChainContext } from './context';

// =============================================================================
// Resolution Result
// =============================================================================

/**
 * Result of resolving a settings value.
 */
export interface ResolvedValue<T> {
  /** The resolved value */
  readonly value: T;

  /** The source of the value */
  readonly source: ResolvedValueSource;

  /** The winning entry (if not from default) */
  readonly winningEntry?: SettingsEntry<T, Scope>;

  /** All entries that were considered (for debugging) */
  readonly consideredEntries: readonly SettingsEntry<unknown, Scope>[];
}

/**
 * Source of a resolved value.
 */
export type ResolvedValueSource =
  | { readonly kind: 'default' }
  | { readonly kind: 'entry'; readonly scope: Scope; readonly isImportant: boolean }
  | { readonly kind: 'merged'; readonly scopeCount: number };

// =============================================================================
// Resolver Interface
// =============================================================================

/**
 * Settings resolver interface.
 */
export interface ISettingsResolver {
  /**
   * Resolves a setting value for a given context.
   *
   * @param key - The key definition
   * @param context - The resolution context
   * @returns The resolved value
   */
  resolve<T>(
    key: AnySettingsKeyDefinition,
    context: ResolutionContext
  ): Promise<Result<ResolvedValue<T>>>;

  /**
   * Resolves multiple settings at once.
   * More efficient than multiple single resolves.
   *
   * @param keys - The key definitions
   * @param context - The resolution context
   * @returns Map of key identifier to resolved value
   */
  resolveMany(
    keys: readonly AnySettingsKeyDefinition[],
    context: ResolutionContext
  ): Promise<Result<ReadonlyMap<string, ResolvedValue<unknown>>>>;

  /**
   * Gets raw entries for a key without resolution.
   *
   * @param keyIdentifier - The key to fetch
   * @param scopes - The scopes to query
   * @returns The raw entries
   */
  getRawEntries(
    keyIdentifier: KeyIdentifier,
    scopes: readonly Scope[]
  ): Promise<readonly SettingsEntry<unknown, Scope>[]>;
}

// =============================================================================
// Resolver Implementation
// =============================================================================

/**
 * Creates a settings resolver.
 *
 * @param persistence - The persistence adapter
 * @returns A settings resolver
 */
export function createResolver(persistence: IPersistenceAdapter): ISettingsResolver {
  /**
   * Converts stored entries to typed entries.
   */
  async function fetchAndDeserializeEntries(
    key: AnySettingsKeyDefinition,
    scopes: readonly Scope[]
  ): Promise<SettingsEntry<unknown, Scope>[]> {
    const storedEntries = await persistence.getEntriesForKey(key.identifier, scopes);

    const entries: SettingsEntry<unknown, Scope>[] = [];
    for (const stored of storedEntries) {
      const result = deserializeEntry(stored, key);
      if ('error' in result) {
        // Log and skip invalid entries
        console.warn(`Failed to deserialize entry for ${key.identifier}: ${result.error}`);
        continue;
      }
      entries.push(result);
    }

    return entries;
  }

  /**
   * Resolves a value using the standard precedence rules.
   */
  function resolveStandard<T>(
    key: AnySettingsKeyDefinition,
    entries: readonly SettingsEntry<unknown, Scope>[]
  ): ResolvedValue<T> {
    // Filter by importance
    const filtered = filterByImportance(entries);

    if (filtered.length === 0) {
      // No entries - use default
      return {
        value: key.valueType.getDefault() as T,
        source: { kind: 'default' },
        consideredEntries: entries,
      };
    }

    // Find the winning entry
    const winner = findWinningEntry(filtered);

    if (!winner) {
      return {
        value: key.valueType.getDefault() as T,
        source: { kind: 'default' },
        consideredEntries: entries,
      };
    }

    return {
      value: winner.value as T,
      source: {
        kind: 'entry',
        scope: winner.scope,
        isImportant: winner.isImportant,
      },
      winningEntry: winner as SettingsEntry<T, Scope>,
      consideredEntries: entries,
    };
  }

  /**
   * Resolves a value using precedence merging.
   */
  function resolveMerged<T>(
    key: AnySettingsKeyDefinition,
    entries: readonly SettingsEntry<unknown, Scope>[]
  ): ResolvedValue<T> {
    const typeId = key.valueType.kind;

    if (!isMergeableType(typeId)) {
      // Fall back to standard resolution if type isn't actually mergeable
      return resolveStandard(key, entries);
    }

    if (entries.length === 0) {
      return {
        value: key.valueType.getDefault() as T,
        source: { kind: 'default' },
        consideredEntries: entries,
      };
    }

    const mergedValue = mergeEntriesWithDefault(
      entries as SettingsEntry<T, Scope>[],
      key.valueType.getDefault() as T,
      typeId
    );

    return {
      value: mergedValue,
      source: { kind: 'merged', scopeCount: entries.length },
      consideredEntries: entries,
    };
  }

  return {
    async resolve<T>(
      key: AnySettingsKeyDefinition,
      context: ResolutionContext
    ): Promise<Result<ResolvedValue<T>>> {
      try {
        // Validate context matches key's settings kind
        if (key.settingsKind !== context.kind) {
          return err(
            `Context kind '${context.kind}' does not match key's settings kind '${key.settingsKind}'`
          );
        }

        // Build the scope chain for this context
        const scopeChainContext = contextToScopeChainContext(context);
        const scopes = buildScopeChain(key.settingsKind, scopeChainContext);

        // Fetch entries (excluding global which is the default)
        const nonGlobalScopes = scopes.filter((s) => s.scope !== 'global');
        const entries = await fetchAndDeserializeEntries(key, nonGlobalScopes);

        // Resolve based on whether this key uses precedence merging
        const resolved = key.precedenceMerging
          ? resolveMerged<T>(key, entries)
          : resolveStandard<T>(key, entries);

        return ok(resolved);
      } catch (error) {
        return err(`Failed to resolve setting: ${error}`);
      }
    },

    async resolveMany(
      keys: readonly AnySettingsKeyDefinition[],
      context: ResolutionContext
    ): Promise<Result<ReadonlyMap<string, ResolvedValue<unknown>>>> {
      try {
        // Build scope chain once
        const scopeChainContext = contextToScopeChainContext(context);

        // Group keys by settings kind
        const keysByKind = new Map<SettingsKind, AnySettingsKeyDefinition[]>();
        for (const key of keys) {
          if (key.settingsKind !== context.kind) {
            return err(
              `Key '${key.identifier}' has settings kind '${key.settingsKind}' but context is '${context.kind}'`
            );
          }
          const existing = keysByKind.get(key.settingsKind) ?? [];
          existing.push(key);
          keysByKind.set(key.settingsKind, existing);
        }

        const results = new Map<string, ResolvedValue<unknown>>();

        // Process each settings kind
        for (const [kind, kindKeys] of Array.from(keysByKind.entries())) {
          const scopes = buildScopeChain(kind, scopeChainContext);
          const nonGlobalScopes = scopes.filter((s) => s.scope !== 'global');

          // Batch fetch entries
          const batchResult = await persistence.getEntriesBatch({
            keyIdentifiers: kindKeys.map((k) => k.identifier),
            scopes: nonGlobalScopes,
          });

          // Resolve each key
          for (const key of kindKeys) {
            const storedEntries = batchResult.get(key.identifier as string) ?? [];

            // Deserialize entries
            const entries: SettingsEntry<unknown, Scope>[] = [];
            for (const stored of storedEntries) {
              const result = deserializeEntry(stored, key);
              if (!('error' in result)) {
                entries.push(result);
              }
            }

            // Resolve
            const resolved = key.precedenceMerging
              ? resolveMerged(key, entries)
              : resolveStandard(key, entries);

            results.set(key.identifier as string, resolved);
          }
        }

        return ok(results);
      } catch (error) {
        return err(`Failed to resolve settings: ${error}`);
      }
    },

    async getRawEntries(
      keyIdentifier: KeyIdentifier,
      scopes: readonly Scope[]
    ): Promise<readonly SettingsEntry<unknown, Scope>[]> {
      const storedEntries = await persistence.getEntriesForKey(keyIdentifier, scopes);

      // We don't have the key definition here, so we return the raw stored entries
      // as basic entries with unknown values
      return storedEntries.map((stored) =>
        createEntry(
          stored.keyIdentifier as KeyIdentifier,
          stored.value,
          {
            kind: stored.scope.kind,
            scope: stored.scope.scope,
            guildId: stored.scope.guildId,
            userId: stored.scope.userId,
          } as Scope,
          stored.isImportant
        )
      );
    },
  };
}
