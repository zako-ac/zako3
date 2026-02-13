/**
 * @fileoverview Guild settings key definitions.
 *
 * These are the standard guild settings keys as defined in the settings documentation.
 */

import {
  KeyIdentifier,
  booleanType,
  voiceChannelFollowingRuleType,
  memberFilterType,
} from '../types';
import { defineGuildKey, type GuildSettingsKeyDefinition } from './definition';

// =============================================================================
// Guild Settings Keys
// =============================================================================

/**
 * How the bot follows voice channels.
 */
export const GUILD_KEY_VC_FOLLOWING_RULE = defineGuildKey({
  identifier: KeyIdentifier('guild.voice.following-rule'),
  friendlyName: 'Voice Channel Following Rule',
  description: 'Chooses how the bot follows voice channels.',
  valueType: voiceChannelFollowingRuleType(),
});

/**
 * Permission filter for /join and /leave commands.
 */
export const GUILD_KEY_JOIN_LEAVE_PERMISSION = defineGuildKey({
  identifier: KeyIdentifier('guild.permissions.join-leave'),
  friendlyName: 'Join Leave Command Permission',
  description: 'Select who can use /join and /leave commands.',
  valueType: memberFilterType(),
});

/**
 * Permission filter for /tts-channel enable and disable commands.
 */
export const GUILD_KEY_ENABLE_DISABLE_PERMISSION = defineGuildKey({
  identifier: KeyIdentifier('guild.permissions.enable-disable'),
  friendlyName: 'Enable Disable Command Permission',
  description: 'Select who can use /tts-channel enable and /tts-channel disable commands.',
  valueType: memberFilterType(),
});

/**
 * Whether users can control bot in channels they don't have access to.
 */
export const GUILD_KEY_JOIN_WITHOUT_PERMISSION = defineGuildKey({
  identifier: KeyIdentifier('guild.permissions.join-without-access'),
  friendlyName: 'Can User Make Bot Join Channel Without Permission',
  description:
    'Whether a user can use /join or /leave, /tts-channel for channels that the user doesn\'t have access to.',
  valueType: booleanType(false),
});

// =============================================================================
// All Guild Keys Collection
// =============================================================================

/**
 * All defined guild settings keys.
 */
export const ALL_GUILD_KEYS: readonly GuildSettingsKeyDefinition<unknown>[] = [
  GUILD_KEY_VC_FOLLOWING_RULE,
  GUILD_KEY_JOIN_LEAVE_PERMISSION,
  GUILD_KEY_ENABLE_DISABLE_PERMISSION,
  GUILD_KEY_JOIN_WITHOUT_PERMISSION,
] as const;
