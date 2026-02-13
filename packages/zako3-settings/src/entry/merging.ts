/**
 * @fileoverview Mergeable trait and merge logic for precedence merging.
 *
 * Precedence merging allows values from multiple scopes to be combined
 * rather than having the highest-precedence scope win completely.
 */

import type { MappingConfig } from '../types';
import { mergeMappingConfigs, emptyMappingConfig } from '../types';
import type { Scope } from '../scope';
import { sortEntriesByPrecedence, filterByImportance } from '../scope';
import type { SettingsEntry } from './entry';

// =============================================================================
// Mergeable Interface
// =============================================================================

/**
 * Interface for types that can be merged with precedence.
 *
 * When precedence merging is enabled for a key, values from all applicable
 * scopes are merged together rather than just taking the highest precedence value.
 *
 * @typeParam T - The type being merged
 */
export interface Mergeable<T> {
  /**
   * Merges this value with another value.
   * The `other` value has higher precedence and should take priority
   * in case of conflicts.
   *
   * @param other - The higher-precedence value to merge with
   * @returns The merged result
   */
  merge(other: T): T;

  /**
   * Returns the identity element for merging.
   * Merging with the identity should return the same value.
   */
  identity(): T;
}

// =============================================================================
// Merge Functions Registry
// =============================================================================

/**
 * Type identifier for mergeable types.
 */
export type MergeableTypeId = 'mappingConfig';

/**
 * Merge function signature.
 */
export type MergeFunction<T> = (base: T, other: T) => T;

/**
 * Identity function signature (returns the empty/default value for merging).
 */
export type IdentityFunction<T> = () => T;

/**
 * Registry of merge functions by type.
 */
const mergeFunctions = new Map<string, MergeFunction<unknown>>();
const identityFunctions = new Map<string, IdentityFunction<unknown>>();

/**
 * Registers merge functions for a type.
 *
 * @param typeId - The type identifier
 * @param merge - The merge function
 * @param identity - The identity function
 */
export function registerMergeableType<T>(
  typeId: string,
  merge: MergeFunction<T>,
  identity: IdentityFunction<T>
): void {
  mergeFunctions.set(typeId, merge as MergeFunction<unknown>);
  identityFunctions.set(typeId, identity as IdentityFunction<unknown>);
}

/**
 * Gets the merge function for a type.
 *
 * @param typeId - The type identifier
 * @returns The merge function or undefined
 */
export function getMergeFunction<T>(typeId: string): MergeFunction<T> | undefined {
  return mergeFunctions.get(typeId) as MergeFunction<T> | undefined;
}

/**
 * Gets the identity function for a type.
 *
 * @param typeId - The type identifier
 * @returns The identity function or undefined
 */
export function getIdentityFunction<T>(typeId: string): IdentityFunction<T> | undefined {
  return identityFunctions.get(typeId) as IdentityFunction<T> | undefined;
}

/**
 * Checks if a type is registered as mergeable.
 *
 * @param typeId - The type identifier
 * @returns True if the type is mergeable
 */
export function isMergeableType(typeId: string): boolean {
  return mergeFunctions.has(typeId);
}

// =============================================================================
// Built-in Mergeable Types
// =============================================================================

// Register MappingConfig as mergeable
registerMergeableType<MappingConfig>(
  'mappingConfig',
  mergeMappingConfigs,
  emptyMappingConfig
);

// =============================================================================
// Entry Merging
// =============================================================================

/**
 * Merges multiple entries for a precedence-merged key.
 *
 * The merging process:
 * 1. Filter by importance (if any important entries exist, non-important are ignored)
 * 2. Sort by precedence
 * 3. Merge values starting from lowest precedence to highest
 *
 * @param entries - The entries to merge
 * @param typeId - The type identifier for the merge function
 * @returns The merged value
 */
export function mergeEntries<T>(
  entries: readonly SettingsEntry<T, Scope>[],
  typeId: string
): T {
  const merge = getMergeFunction<T>(typeId);
  const identity = getIdentityFunction<T>(typeId);

  if (!merge || !identity) {
    throw new Error(`No merge function registered for type: ${typeId}`);
  }

  if (entries.length === 0) {
    return identity();
  }

  // Filter by importance
  const filtered = filterByImportance(entries);

  // Sort by precedence (ascending, so we can fold from left)
  const sorted = sortEntriesByPrecedence(filtered).reverse();

  // Merge from lowest precedence to highest
  let result = identity();
  for (const entry of sorted) {
    result = merge(result, entry.value);
  }

  return result;
}

/**
 * Merges entries with a base default value.
 *
 * @param entries - The entries to merge
 * @param defaultValue - The default value to start with
 * @param typeId - The type identifier for the merge function
 * @returns The merged value
 */
export function mergeEntriesWithDefault<T>(
  entries: readonly SettingsEntry<T, Scope>[],
  defaultValue: T,
  typeId: string
): T {
  const merge = getMergeFunction<T>(typeId);

  if (!merge) {
    throw new Error(`No merge function registered for type: ${typeId}`);
  }

  if (entries.length === 0) {
    return defaultValue;
  }

  // Filter by importance
  const filtered = filterByImportance(entries);

  // Sort by precedence (ascending, so we can fold from left)
  const sorted = sortEntriesByPrecedence(filtered).reverse();

  // Start with default and merge all entries
  let result = defaultValue;
  for (const entry of sorted) {
    result = merge(result, entry.value);
  }

  return result;
}

// =============================================================================
// Utility Types
// =============================================================================

/**
 * Type guard to check if a value type descriptor indicates mergeability.
 */
export function isValueTypeMergeable(valueType: { kind: string }): boolean {
  return isMergeableType(valueType.kind);
}
