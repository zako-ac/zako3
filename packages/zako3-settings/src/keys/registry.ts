/**
 * @fileoverview Key registry for runtime lookup of settings keys.
 *
 * The registry provides a centralized location to register and retrieve
 * settings key definitions by their identifier.
 */

import type { KeyIdentifier } from '../types';
import type { SettingsKind } from '../scope';
import type {
  AnySettingsKeyDefinition,
  SettingsKeyDefinition,
  UserSettingsKeyDefinition,
  GuildSettingsKeyDefinition,
  AdminSettingsKeyDefinition,
} from './definition';

// =============================================================================
// Registry Interface
// =============================================================================

/**
 * Read-only view of the key registry.
 */
export interface KeyRegistryReader {
  /** Gets a key definition by identifier */
  get(identifier: KeyIdentifier): AnySettingsKeyDefinition | undefined;

  /** Gets a key definition, throwing if not found */
  getOrThrow(identifier: KeyIdentifier): AnySettingsKeyDefinition;

  /** Checks if a key exists */
  has(identifier: KeyIdentifier): boolean;

  /** Gets all registered keys */
  getAll(): readonly AnySettingsKeyDefinition[];

  /** Gets all keys of a specific settings kind */
  getByKind(kind: SettingsKind): readonly AnySettingsKeyDefinition[];

  /** Gets all user setting keys */
  getUserKeys(): readonly UserSettingsKeyDefinition[];

  /** Gets all guild setting keys */
  getGuildKeys(): readonly GuildSettingsKeyDefinition[];

  /** Gets all admin setting keys */
  getAdminKeys(): readonly AdminSettingsKeyDefinition[];

  /** Gets the number of registered keys */
  readonly size: number;
}

/**
 * Mutable key registry for registration.
 */
export interface KeyRegistryWriter {
  /** Registers a key definition */
  register(key: AnySettingsKeyDefinition): void;

  /** Registers multiple key definitions */
  registerMany(keys: readonly AnySettingsKeyDefinition[]): void;

  /** Unregisters a key definition */
  unregister(identifier: KeyIdentifier): boolean;

  /** Clears all registered keys */
  clear(): void;
}

/**
 * Full key registry interface.
 */
export interface KeyRegistry extends KeyRegistryReader, KeyRegistryWriter {}

// =============================================================================
// Registry Implementation
// =============================================================================

/**
 * Creates a new key registry.
 *
 * @returns A new empty key registry
 */
export function createKeyRegistry(): KeyRegistry {
  const keys = new Map<string, AnySettingsKeyDefinition>();

  return {
    // Reader methods
    get(identifier: KeyIdentifier): AnySettingsKeyDefinition | undefined {
      return keys.get(identifier as string);
    },

    getOrThrow(identifier: KeyIdentifier): AnySettingsKeyDefinition {
      const key = keys.get(identifier as string);
      if (!key) {
        throw new Error(`Settings key not found: ${identifier}`);
      }
      return key;
    },

    has(identifier: KeyIdentifier): boolean {
      return keys.has(identifier as string);
    },

    getAll(): readonly AnySettingsKeyDefinition[] {
      return Array.from(keys.values());
    },

    getByKind(kind: SettingsKind): readonly AnySettingsKeyDefinition[] {
      return Array.from(keys.values()).filter((k) => k.settingsKind === kind);
    },

    getUserKeys(): readonly UserSettingsKeyDefinition[] {
      return Array.from(keys.values()).filter(
        (k): k is UserSettingsKeyDefinition => k.settingsKind === 'user'
      );
    },

    getGuildKeys(): readonly GuildSettingsKeyDefinition[] {
      return Array.from(keys.values()).filter(
        (k): k is GuildSettingsKeyDefinition => k.settingsKind === 'guild'
      );
    },

    getAdminKeys(): readonly AdminSettingsKeyDefinition[] {
      return Array.from(keys.values()).filter(
        (k): k is AdminSettingsKeyDefinition => k.settingsKind === 'admin'
      );
    },

    get size(): number {
      return keys.size;
    },

    // Writer methods
    register(key: AnySettingsKeyDefinition): void {
      const id = key.identifier as string;
      if (keys.has(id)) {
        throw new Error(`Settings key already registered: ${id}`);
      }
      keys.set(id, key);
    },

    registerMany(keyList: readonly AnySettingsKeyDefinition[]): void {
      // Validate all keys first
      for (const key of keyList) {
        const id = key.identifier as string;
        if (keys.has(id)) {
          throw new Error(`Settings key already registered: ${id}`);
        }
      }
      // Then register all
      for (const key of keyList) {
        keys.set(key.identifier as string, key);
      }
    },

    unregister(identifier: KeyIdentifier): boolean {
      return keys.delete(identifier as string);
    },

    clear(): void {
      keys.clear();
    },
  };
}

// =============================================================================
// Global Registry (Optional Singleton)
// =============================================================================

let globalRegistry: KeyRegistry | null = null;

/**
 * Gets the global key registry instance.
 * Creates one if it doesn't exist.
 *
 * @returns The global key registry
 */
export function getGlobalRegistry(): KeyRegistry {
  if (!globalRegistry) {
    globalRegistry = createKeyRegistry();
  }
  return globalRegistry;
}

/**
 * Resets the global registry (useful for testing).
 */
export function resetGlobalRegistry(): void {
  globalRegistry = null;
}

// =============================================================================
// Type-safe Key Access
// =============================================================================

/**
 * Type-safe wrapper for accessing a specific key's value type.
 * Useful for creating strongly-typed accessor functions.
 *
 * @example
 * ```typescript
 * const TTS_VOICE_KEY = defineUserKey({
 *   identifier: KeyIdentifier('tts.voice.default'),
 *   ...
 * });
 *
 * type TTSVoiceValue = KeyValueType<typeof TTS_VOICE_KEY>;
 * ```
 */
export type KeyValueType<K extends AnySettingsKeyDefinition> =
  K extends SettingsKeyDefinition<infer T, SettingsKind> ? T : never;

/**
 * Extracts the settings kind from a key definition type.
 */
export type KeySettingsKind<K extends AnySettingsKeyDefinition> =
  K extends SettingsKeyDefinition<unknown, infer S> ? S : never;
