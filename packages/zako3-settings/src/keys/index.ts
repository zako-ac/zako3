/**
 * @fileoverview Keys module exports.
 */

// Key definition types and builders
export {
  type SettingsKeyDefinition,
  type UserSettingsKeyDefinition,
  type GuildSettingsKeyDefinition,
  type AdminSettingsKeyDefinition,
  type AnySettingsKeyDefinition,
  defineUserKey,
  defineGuildKey,
  defineAdminKey,
  getKeyDefaultValue,
  validateKeyValue,
  serializeKeyValue,
  deserializeKeyValue,
  isScopeAllowedForUserKey,
  isUserKey,
  isGuildKey,
  isAdminKey,
} from './definition';

// Key registry
export {
  type KeyRegistryReader,
  type KeyRegistryWriter,
  type KeyRegistry,
  type KeyValueType,
  type KeySettingsKind,
  createKeyRegistry,
  getGlobalRegistry,
  resetGlobalRegistry,
} from './registry';

// Pre-defined user keys
export {
  USER_KEY_MAPPINGS,
  USER_KEY_READ_TEXT_NOT_IN_VC,
  USER_KEY_TTS_VOICE,
  USER_KEY_JOIN_ALERT_NAME,
  USER_KEY_STOP_TTS_KEYWORDS,
  USER_KEY_ALLOW_VOLUME_OVER_LIMIT,
  ALL_USER_KEYS,
} from './user-keys';

// Pre-defined guild keys
export {
  GUILD_KEY_VC_FOLLOWING_RULE,
  GUILD_KEY_JOIN_LEAVE_PERMISSION,
  GUILD_KEY_ENABLE_DISABLE_PERMISSION,
  GUILD_KEY_JOIN_WITHOUT_PERMISSION,
  ALL_GUILD_KEYS,
} from './guild-keys';

// Pre-defined admin keys
export {
  ADMIN_KEY_ADMINS,
  ALL_ADMIN_KEYS,
} from './admin-keys';
