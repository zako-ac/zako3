/**
 * @fileoverview Discord-compatible identifier types using the newtype pattern.
 *
 * Discord uses "Snowflake" IDs which are 64-bit unsigned integers represented
 * as strings. These branded types provide type safety while maintaining
 * compatibility with Discord.js and other Discord libraries.
 */

import { Brand, makeBrandConstructor, makeBrandFactory, type BrandResult } from './brand';

// =============================================================================
// Core Identifier Types
// =============================================================================

/**
 * Discord User ID (Snowflake).
 * A unique identifier for a Discord user.
 */
export type UserId = Brand<string, 'UserId'>;

/**
 * Discord Guild ID (Snowflake).
 * A unique identifier for a Discord guild (server).
 */
export type GuildId = Brand<string, 'GuildId'>;

/**
 * Discord Channel ID (Snowflake).
 * A unique identifier for a Discord channel.
 */
export type ChannelId = Brand<string, 'ChannelId'>;

/**
 * Discord Role ID (Snowflake).
 * A unique identifier for a Discord role.
 */
export type RoleId = Brand<string, 'RoleId'>;

/**
 * Discord Emoji ID (Snowflake).
 * A unique identifier for a custom Discord emoji.
 */
export type EmojiId = Brand<string, 'EmojiId'>;

/**
 * Discord Sticker ID (Snowflake).
 * A unique identifier for a Discord sticker.
 */
export type StickerId = Brand<string, 'StickerId'>;

/**
 * Settings Key Identifier.
 * Format: `<tab>.<category>.<name>` (e.g., "tts.voice.default")
 */
export type KeyIdentifier = Brand<string, 'KeyIdentifier'>;

/**
 * Tap Reference.
 * A reference to a TTS tap (e.g., "google", "azure").
 */
export type TapRef = Brand<string, 'TapRef'>;

// =============================================================================
// Validation Helpers
// =============================================================================

/**
 * Validates that a string is a valid Discord Snowflake.
 * Snowflakes are numeric strings representing 64-bit unsigned integers.
 *
 * @param value - The value to validate
 * @returns Error message if invalid, null if valid
 */
function validateSnowflake(value: string): string | null {
  if (!/^\d{17,20}$/.test(value)) {
    return 'Invalid Snowflake: must be a 17-20 digit numeric string';
  }
  return null;
}

/**
 * Validates that a string is a valid key identifier.
 * Format: `<tab>.<category>.<name>` where each part is alphanumeric with hyphens.
 *
 * @param value - The value to validate
 * @returns Error message if invalid, null if valid
 */
function validateKeyIdentifier(value: string): string | null {
  const parts = value.split('.');
  if (parts.length !== 3) {
    return 'Invalid key identifier: must have exactly 3 parts separated by dots';
  }
  const validPart = /^[a-z][a-z0-9-]*$/;
  for (const part of parts) {
    if (!validPart.test(part)) {
      return `Invalid key identifier part "${part}": must be lowercase alphanumeric with hyphens, starting with a letter`;
    }
  }
  return null;
}

/**
 * Validates that a string is a valid tap reference.
 *
 * @param value - The value to validate
 * @returns Error message if invalid, null if valid
 */
function validateTapRef(value: string): string | null {
  if (!/^[a-z][a-z0-9-]*$/.test(value)) {
    return 'Invalid tap reference: must be lowercase alphanumeric with hyphens, starting with a letter';
  }
  return null;
}

// =============================================================================
// Simple Constructors (No Validation)
// =============================================================================

/**
 * Creates a UserId without validation.
 * Use this when you trust the source (e.g., from Discord API).
 */
export const UserId = makeBrandConstructor<string, 'UserId'>();

/**
 * Creates a GuildId without validation.
 * Use this when you trust the source (e.g., from Discord API).
 */
export const GuildId = makeBrandConstructor<string, 'GuildId'>();

/**
 * Creates a ChannelId without validation.
 * Use this when you trust the source (e.g., from Discord API).
 */
export const ChannelId = makeBrandConstructor<string, 'ChannelId'>();

/**
 * Creates a RoleId without validation.
 * Use this when you trust the source (e.g., from Discord API).
 */
export const RoleId = makeBrandConstructor<string, 'RoleId'>();

/**
 * Creates an EmojiId without validation.
 * Use this when you trust the source (e.g., from Discord API).
 */
export const EmojiId = makeBrandConstructor<string, 'EmojiId'>();

/**
 * Creates a StickerId without validation.
 * Use this when you trust the source (e.g., from Discord API).
 */
export const StickerId = makeBrandConstructor<string, 'StickerId'>();

/**
 * Creates a KeyIdentifier without validation.
 * Use this for statically defined keys.
 */
export const KeyIdentifier = makeBrandConstructor<string, 'KeyIdentifier'>();

/**
 * Creates a TapRef without validation.
 * Use this for statically defined tap references.
 */
export const TapRef = makeBrandConstructor<string, 'TapRef'>();

// =============================================================================
// Validating Factories
// =============================================================================

/**
 * Creates a UserId with Snowflake validation.
 *
 * @param value - The string to convert to UserId
 * @returns Result with UserId or error message
 */
export const parseUserId = makeBrandFactory<string, 'UserId'>(validateSnowflake);

/**
 * Creates a GuildId with Snowflake validation.
 *
 * @param value - The string to convert to GuildId
 * @returns Result with GuildId or error message
 */
export const parseGuildId = makeBrandFactory<string, 'GuildId'>(validateSnowflake);

/**
 * Creates a ChannelId with Snowflake validation.
 *
 * @param value - The string to convert to ChannelId
 * @returns Result with ChannelId or error message
 */
export const parseChannelId = makeBrandFactory<string, 'ChannelId'>(validateSnowflake);

/**
 * Creates a RoleId with Snowflake validation.
 *
 * @param value - The string to convert to RoleId
 * @returns Result with RoleId or error message
 */
export const parseRoleId = makeBrandFactory<string, 'RoleId'>(validateSnowflake);

/**
 * Creates an EmojiId with Snowflake validation.
 *
 * @param value - The string to convert to EmojiId
 * @returns Result with EmojiId or error message
 */
export const parseEmojiId = makeBrandFactory<string, 'EmojiId'>(validateSnowflake);

/**
 * Creates a StickerId with Snowflake validation.
 *
 * @param value - The string to convert to StickerId
 * @returns Result with StickerId or error message
 */
export const parseStickerId = makeBrandFactory<string, 'StickerId'>(validateSnowflake);

/**
 * Creates a KeyIdentifier with format validation.
 *
 * @param value - The string to convert to KeyIdentifier
 * @returns Result with KeyIdentifier or error message
 */
export const parseKeyIdentifier = makeBrandFactory<string, 'KeyIdentifier'>(validateKeyIdentifier);

/**
 * Creates a TapRef with format validation.
 *
 * @param value - The string to convert to TapRef
 * @returns Result with TapRef or error message
 */
export const parseTapRef = makeBrandFactory<string, 'TapRef'>(validateTapRef);

// =============================================================================
// Utility Types
// =============================================================================

/**
 * Extracts the tab from a KeyIdentifier.
 */
export function getKeyTab(key: KeyIdentifier): string {
  return (key as string).split('.')[0];
}

/**
 * Extracts the category from a KeyIdentifier.
 */
export function getKeyCategory(key: KeyIdentifier): string {
  return (key as string).split('.')[1];
}

/**
 * Extracts the name from a KeyIdentifier.
 */
export function getKeyName(key: KeyIdentifier): string {
  return (key as string).split('.')[2];
}

/**
 * Composes a KeyIdentifier from its parts.
 *
 * @param tab - The tab part
 * @param category - The category part
 * @param name - The name part
 * @returns Result with KeyIdentifier or error message
 */
export function composeKeyIdentifier(
  tab: string,
  category: string,
  name: string
): BrandResult<string, 'KeyIdentifier'> {
  return parseKeyIdentifier(`${tab}.${category}.${name}`);
}
