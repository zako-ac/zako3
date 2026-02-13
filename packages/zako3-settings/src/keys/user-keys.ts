/**
 * @fileoverview User settings key definitions.
 *
 * These are the standard user settings keys as defined in the settings documentation.
 */

import {
  KeyIdentifier,
  booleanType,
  stringType,
  listType,
  someOrDefaultType,
  mappingConfigType,
  tapRefType,
  TapRef,
} from '../types';
import { defineUserKey, type UserSettingsKeyDefinition } from './definition';

// =============================================================================
// User Settings Keys
// =============================================================================

/**
 * Mappings configuration for text, emoji, and sticker transformations.
 * Supports precedence merging across scopes.
 */
export const USER_KEY_MAPPINGS = defineUserKey({
  identifier: KeyIdentifier('user.tts.mappings'),
  friendlyName: 'Mappings',
  description: 'Configuration about mappings for text, emoji, and sticker transformations.',
  valueType: mappingConfigType(),
  precedenceMerging: true,
});

/**
 * Whether to read text even when the user is not in a voice channel.
 */
export const USER_KEY_READ_TEXT_NOT_IN_VC = defineUserKey({
  identifier: KeyIdentifier('user.tts.read-not-in-vc'),
  friendlyName: 'Read Text Even Not In Voice Channel',
  description: 'Whether to read text even when the user is not in a voice channel.',
  valueType: booleanType(true),
});

/**
 * Selected TTS voice/tap.
 */
export const USER_KEY_TTS_VOICE = defineUserKey({
  identifier: KeyIdentifier('user.tts.voice'),
  friendlyName: 'TTS Voice',
  description: 'Selected TTS Tap for text-to-speech.',
  valueType: tapRefType(TapRef('google')),
});

/**
 * Custom name for join alerts.
 * Uses SomeOrDefault to allow "use default" option.
 */
export const USER_KEY_JOIN_ALERT_NAME = defineUserKey({
  identifier: KeyIdentifier('user.alert.join-name'),
  friendlyName: 'User Join Alert Name',
  description: 'Name of the user to use on the join alert. Defaults to member nickname or username.',
  valueType: someOrDefaultType(stringType('', { maxLength: 100 })),
});

/**
 * Keywords that will stop the TTS.
 */
export const USER_KEY_STOP_TTS_KEYWORDS = defineUserKey({
  identifier: KeyIdentifier('user.tts.stop-keywords'),
  friendlyName: 'Stop TTS Keywords',
  description: 'List of keywords that will stop the TTS when spoken.',
  valueType: listType(stringType(), { defaultValue: ['닥쳐'] }),
});

/**
 * Whether the user can set volume over 100%.
 * Admin only setting.
 */
export const USER_KEY_ALLOW_VOLUME_OVER_LIMIT = defineUserKey({
  identifier: KeyIdentifier('user.audio.allow-volume-over-limit'),
  friendlyName: 'Allow Volume Over Limit',
  description: 'Whether allow the user to set volume over 100%.',
  valueType: booleanType(false),
  adminOnly: true,
});

// =============================================================================
// All User Keys Collection
// =============================================================================

/**
 * All defined user settings keys.
 */
export const ALL_USER_KEYS: readonly UserSettingsKeyDefinition<unknown>[] = [
  USER_KEY_MAPPINGS,
  USER_KEY_READ_TEXT_NOT_IN_VC,
  USER_KEY_TTS_VOICE,
  USER_KEY_JOIN_ALERT_NAME,
  USER_KEY_STOP_TTS_KEYWORDS,
  USER_KEY_ALLOW_VOLUME_OVER_LIMIT,
] as const;
