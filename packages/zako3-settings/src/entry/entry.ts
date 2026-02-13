/**
 * @fileoverview Settings entry types and utilities.
 *
 * A settings entry represents a stored value for a specific key at a specific scope.
 */

import type { KeyIdentifier } from '../types';
import type { Scope, UserScope, GuildScope, AdminScope, SettingsKind } from '../scope';
import type { AnySettingsKeyDefinition } from '../keys/definition';

// =============================================================================
// Entry Types
// =============================================================================

/**
 * Base settings entry interface.
 *
 * @typeParam T - The type of the stored value
 * @typeParam S - The scope type
 */
export interface SettingsEntry<T = unknown, S extends Scope = Scope> {
  /** The key identifier this entry belongs to */
  readonly keyIdentifier: KeyIdentifier;

  /** The stored value */
  readonly value: T;

  /** The scope at which this value is set */
  readonly scope: S;

  /** Whether this is an important entry (inverts precedence) */
  readonly isImportant: boolean;
}

/**
 * Entry for user settings.
 */
export type UserSettingsEntry<T = unknown> = SettingsEntry<T, UserScope>;

/**
 * Entry for guild settings.
 */
export type GuildSettingsEntry<T = unknown> = SettingsEntry<T, GuildScope>;

/**
 * Entry for admin settings.
 */
export type AdminSettingsEntry<T = unknown> = SettingsEntry<T, AdminScope>;

/**
 * Union of all entry types.
 */
export type AnySettingsEntry =
  | UserSettingsEntry<unknown>
  | GuildSettingsEntry<unknown>
  | AdminSettingsEntry<unknown>;

// =============================================================================
// Entry Constructors
// =============================================================================

/**
 * Creates a settings entry.
 *
 * @param keyIdentifier - The key this entry is for
 * @param value - The stored value
 * @param scope - The scope at which this is set
 * @param isImportant - Whether this is an important entry
 * @returns A frozen settings entry
 */
export function createEntry<T, S extends Scope>(
  keyIdentifier: KeyIdentifier,
  value: T,
  scope: S,
  isImportant: boolean = false
): SettingsEntry<T, S> {
  return Object.freeze({
    keyIdentifier,
    value,
    scope,
    isImportant,
  });
}

/**
 * Creates a user settings entry.
 */
export function createUserEntry<T>(
  keyIdentifier: KeyIdentifier,
  value: T,
  scope: UserScope,
  isImportant: boolean = false
): UserSettingsEntry<T> {
  return createEntry(keyIdentifier, value, scope, isImportant);
}

/**
 * Creates a guild settings entry.
 */
export function createGuildEntry<T>(
  keyIdentifier: KeyIdentifier,
  value: T,
  scope: GuildScope,
  isImportant: boolean = false
): GuildSettingsEntry<T> {
  return createEntry(keyIdentifier, value, scope, isImportant);
}

/**
 * Creates an admin settings entry.
 */
export function createAdminEntry<T>(
  keyIdentifier: KeyIdentifier,
  value: T,
  scope: AdminScope,
  isImportant: boolean = false
): AdminSettingsEntry<T> {
  return createEntry(keyIdentifier, value, scope, isImportant);
}

// =============================================================================
// Entry Utilities
// =============================================================================

/**
 * Creates a copy of an entry with a new value.
 */
export function withValue<T, S extends Scope>(
  entry: SettingsEntry<unknown, S>,
  value: T
): SettingsEntry<T, S> {
  return createEntry(entry.keyIdentifier, value, entry.scope, entry.isImportant);
}

/**
 * Creates a copy of an entry with a new importance flag.
 */
export function withImportance<T, S extends Scope>(
  entry: SettingsEntry<T, S>,
  isImportant: boolean
): SettingsEntry<T, S> {
  return createEntry(entry.keyIdentifier, entry.value, entry.scope, isImportant);
}

/**
 * Gets the settings kind from an entry's scope.
 */
export function getEntryKind(entry: AnySettingsEntry): SettingsKind {
  return entry.scope.kind;
}

// =============================================================================
// Stored Entry (for persistence)
// =============================================================================

/**
 * Serializable representation of an entry for storage.
 * The value is already serialized by the key's value type descriptor.
 */
export interface StoredEntry {
  /** The key identifier */
  readonly keyIdentifier: string;

  /** Serialized value (JSON-compatible) */
  readonly value: unknown;

  /** Serialized scope */
  readonly scope: StoredScope;

  /** Whether this is an important entry */
  readonly isImportant: boolean;
}

/**
 * Serialized scope representation.
 */
export interface StoredScope {
  readonly kind: SettingsKind;
  readonly scope: string;
  readonly guildId?: string;
  readonly userId?: string;
}

/**
 * Converts a scope to its stored representation.
 */
export function serializeScope(scope: Scope): StoredScope {
  const stored: StoredScope = {
    kind: scope.kind,
    scope: scope.scope,
  };

  if ('guildId' in scope) {
    (stored as { guildId?: string }).guildId = scope.guildId as string;
  }
  if ('userId' in scope) {
    (stored as { userId?: string }).userId = scope.userId as string;
  }

  return stored;
}

/**
 * Converts a stored scope back to a Scope type.
 * Note: The branded types are cast here since we're deserializing from storage.
 */
export function deserializeScope(stored: StoredScope): Scope {
  switch (stored.kind) {
    case 'user':
      switch (stored.scope) {
        case 'global':
          return { kind: 'user', scope: 'global' };
        case 'guild':
          return { kind: 'user', scope: 'guild', guildId: stored.guildId as any };
        case 'user':
          return { kind: 'user', scope: 'user', userId: stored.userId as any };
        case 'perGuildUser':
          return {
            kind: 'user',
            scope: 'perGuildUser',
            guildId: stored.guildId as any,
            userId: stored.userId as any,
          };
        default:
          throw new Error(`Unknown user scope: ${stored.scope}`);
      }
    case 'guild':
      switch (stored.scope) {
        case 'global':
          return { kind: 'guild', scope: 'global' };
        case 'guild':
          return { kind: 'guild', scope: 'guild', guildId: stored.guildId as any };
        default:
          throw new Error(`Unknown guild scope: ${stored.scope}`);
      }
    case 'admin':
      return { kind: 'admin', scope: 'admin' };
    default:
      throw new Error(`Unknown scope kind: ${stored.kind}`);
  }
}

/**
 * Serializes an entry for storage.
 *
 * @param entry - The entry to serialize
 * @param keyDef - The key definition (for value serialization)
 * @returns Serialized entry
 */
export function serializeEntry<T>(
  entry: SettingsEntry<T, Scope>,
  keyDef: AnySettingsKeyDefinition
): StoredEntry {
  return {
    keyIdentifier: entry.keyIdentifier as string,
    value: keyDef.valueType.serialize(entry.value),
    scope: serializeScope(entry.scope),
    isImportant: entry.isImportant,
  };
}

/**
 * Deserializes an entry from storage.
 *
 * @param stored - The stored entry
 * @param keyDef - The key definition (for value deserialization)
 * @returns Result with deserialized entry or error
 */
export function deserializeEntry(
  stored: StoredEntry,
  keyDef: AnySettingsKeyDefinition
): SettingsEntry<unknown, Scope> | { error: string } {
  const valueResult = keyDef.valueType.deserialize(stored.value);
  if (!valueResult.ok) {
    return { error: `Failed to deserialize value: ${valueResult.error}` };
  }

  const scope = deserializeScope(stored.scope);
  return createEntry(
    stored.keyIdentifier as KeyIdentifier,
    valueResult.value,
    scope,
    stored.isImportant
  );
}
