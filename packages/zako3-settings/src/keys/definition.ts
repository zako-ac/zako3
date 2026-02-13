/**
 * @fileoverview Settings key definitions and builder.
 *
 * A settings key definition describes a configurable setting, including
 * its identifier, type, default value, and behavioral flags.
 */

import type { KeyIdentifier, ValueTypeDescriptor } from '../types';
import type { SettingsKind, UserScopeId } from '../scope';

// =============================================================================
// Key Definition Interface
// =============================================================================

/**
 * Base interface for all settings key definitions.
 *
 * @typeParam T - The type of the setting value
 * @typeParam K - The settings kind (user, guild, admin)
 */
export interface SettingsKeyDefinition<T = unknown, K extends SettingsKind = SettingsKind> {
  /** Unique identifier in format `<tab>.<category>.<name>` */
  readonly identifier: KeyIdentifier;

  /** The settings kind this key belongs to */
  readonly settingsKind: K;

  /** Human-readable name for UI display */
  readonly friendlyName: string;

  /** Description of what this setting controls */
  readonly description: string;

  /** Type descriptor with validation and serialization logic */
  readonly valueType: ValueTypeDescriptor<T>;

  /** Whether values from multiple scopes should be merged */
  readonly precedenceMerging: boolean;

  /** Whether only admins can modify this setting */
  readonly adminOnly: boolean;
}

/**
 * User settings key definition with additional scope restrictions.
 */
export interface UserSettingsKeyDefinition<T = unknown>
  extends SettingsKeyDefinition<T, 'user'> {
  /**
   * Allowed scopes for this setting.
   * If undefined, all scopes are allowed.
   */
  readonly allowedScopes?: readonly UserScopeId[];
}

/**
 * Guild settings key definition.
 */
export interface GuildSettingsKeyDefinition<T = unknown>
  extends SettingsKeyDefinition<T, 'guild'> {}

/**
 * Admin settings key definition.
 */
export interface AdminSettingsKeyDefinition<T = unknown>
  extends SettingsKeyDefinition<T, 'admin'> {}

/**
 * Union of all key definition types.
 */
export type AnySettingsKeyDefinition =
  | UserSettingsKeyDefinition<unknown>
  | GuildSettingsKeyDefinition<unknown>
  | AdminSettingsKeyDefinition<unknown>;

// =============================================================================
// Key Definition Builder
// =============================================================================

/**
 * Builder configuration for creating key definitions.
 */
interface KeyBuilderConfig<T, K extends SettingsKind> {
  identifier: KeyIdentifier;
  settingsKind: K;
  friendlyName: string;
  description: string;
  valueType: ValueTypeDescriptor<T>;
  precedenceMerging?: boolean;
  adminOnly?: boolean;
  allowedScopes?: readonly UserScopeId[];
}

/**
 * Creates a user settings key definition.
 *
 * @param config - Key configuration
 * @returns A frozen key definition
 */
export function defineUserKey<T>(
  config: Omit<KeyBuilderConfig<T, 'user'>, 'settingsKind'>
): UserSettingsKeyDefinition<T> {
  const definition: UserSettingsKeyDefinition<T> = {
    identifier: config.identifier,
    settingsKind: 'user',
    friendlyName: config.friendlyName,
    description: config.description,
    valueType: config.valueType,
    precedenceMerging: config.precedenceMerging ?? false,
    adminOnly: config.adminOnly ?? false,
    allowedScopes: config.allowedScopes,
  };
  return Object.freeze(definition);
}

/**
 * Creates a guild settings key definition.
 *
 * @param config - Key configuration
 * @returns A frozen key definition
 */
export function defineGuildKey<T>(
  config: Omit<KeyBuilderConfig<T, 'guild'>, 'settingsKind' | 'allowedScopes'>
): GuildSettingsKeyDefinition<T> {
  const definition: GuildSettingsKeyDefinition<T> = {
    identifier: config.identifier,
    settingsKind: 'guild',
    friendlyName: config.friendlyName,
    description: config.description,
    valueType: config.valueType,
    precedenceMerging: config.precedenceMerging ?? false,
    adminOnly: config.adminOnly ?? false,
  };
  return Object.freeze(definition);
}

/**
 * Creates an admin settings key definition.
 *
 * @param config - Key configuration
 * @returns A frozen key definition
 */
export function defineAdminKey<T>(
  config: Omit<KeyBuilderConfig<T, 'admin'>, 'settingsKind' | 'allowedScopes' | 'adminOnly'>
): AdminSettingsKeyDefinition<T> {
  const definition: AdminSettingsKeyDefinition<T> = {
    identifier: config.identifier,
    settingsKind: 'admin',
    friendlyName: config.friendlyName,
    description: config.description,
    valueType: config.valueType,
    precedenceMerging: config.precedenceMerging ?? false,
    adminOnly: true, // Admin keys are always admin-only
  };
  return Object.freeze(definition);
}

// =============================================================================
// Key Definition Utilities
// =============================================================================

/**
 * Gets the default value from a key definition.
 *
 * @param key - The key definition
 * @returns The default value for this key
 */
export function getKeyDefaultValue<T>(key: SettingsKeyDefinition<T>): T {
  return key.valueType.getDefault();
}

/**
 * Validates a value against a key's type descriptor.
 *
 * @param key - The key definition
 * @param value - The value to validate
 * @returns Result with validated value or error
 */
export function validateKeyValue<T>(
  key: SettingsKeyDefinition<T>,
  value: unknown
): ReturnType<ValueTypeDescriptor<T>['validate']> {
  return key.valueType.validate(value);
}

/**
 * Serializes a value for storage.
 *
 * @param key - The key definition
 * @param value - The value to serialize
 * @returns JSON-compatible serialized value
 */
export function serializeKeyValue<T>(
  key: SettingsKeyDefinition<T>,
  value: T
): unknown {
  return key.valueType.serialize(value);
}

/**
 * Deserializes a value from storage.
 *
 * @param key - The key definition
 * @param data - The stored data
 * @returns Result with deserialized value or error
 */
export function deserializeKeyValue<T>(
  key: SettingsKeyDefinition<T>,
  data: unknown
): ReturnType<ValueTypeDescriptor<T>['deserialize']> {
  return key.valueType.deserialize(data);
}

/**
 * Checks if a scope is allowed for a user settings key.
 *
 * @param key - The user key definition
 * @param scopeId - The scope to check
 * @returns True if the scope is allowed
 */
export function isScopeAllowedForUserKey(
  key: UserSettingsKeyDefinition<unknown>,
  scopeId: UserScopeId
): boolean {
  if (!key.allowedScopes) {
    return true; // All scopes allowed by default
  }
  return key.allowedScopes.includes(scopeId);
}

/**
 * Type guard to check if a key definition is for user settings.
 */
export function isUserKey(
  key: AnySettingsKeyDefinition
): key is UserSettingsKeyDefinition<unknown> {
  return key.settingsKind === 'user';
}

/**
 * Type guard to check if a key definition is for guild settings.
 */
export function isGuildKey(
  key: AnySettingsKeyDefinition
): key is GuildSettingsKeyDefinition<unknown> {
  return key.settingsKind === 'guild';
}

/**
 * Type guard to check if a key definition is for admin settings.
 */
export function isAdminKey(
  key: AnySettingsKeyDefinition
): key is AdminSettingsKeyDefinition<unknown> {
  return key.settingsKind === 'admin';
}
