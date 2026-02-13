/**
 * @fileoverview zako3-settings - A modular, extensible settings system.
 *
 * This package provides a comprehensive settings management system with:
 * - Type-safe branded types (newtype pattern)
 * - Scope-based cascading settings (user, guild, admin)
 * - Precedence merging for complex value types
 * - Pluggable persistence layer (DI-ready)
 *
 * @example
 * ```typescript
 * import {
 *   createSettingsManager,
 *   createInMemoryAdapter,
 *   UserId,
 *   GuildId,
 *   userContext,
 *   USER_KEY_TTS_VOICE,
 *   actorUser,
 *   userScopeUser,
 * } from 'zako3-settings';
 *
 * // Create the manager with an in-memory adapter (for testing)
 * const manager = createSettingsManager({
 *   persistence: createInMemoryAdapter(),
 *   keys: [USER_KEY_TTS_VOICE],
 * });
 *
 * await manager.initialize();
 *
 * // Get a setting
 * const result = await manager.get(
 *   USER_KEY_TTS_VOICE,
 *   userContext(UserId('123456789'), GuildId('987654321'))
 * );
 *
 * if (result.ok) {
 *   console.log('TTS Voice:', result.value.value);
 * }
 *
 * // Set a setting
 * await manager.set(
 *   USER_KEY_TTS_VOICE,
 *   TapRef('azure'),
 *   userScopeUser(UserId('123456789')),
 *   actorUser(UserId('123456789'))
 * );
 * ```
 *
 * @packageDocumentation
 */

// =============================================================================
// Types
// =============================================================================

// Brand utilities
export {
  type Brand,
  type Unbrand,
  type BrandOf,
  type BrandResult,
  brand,
  unbrand,
  isBrandable,
  makeBrandFactory,
  makeBrandConstructor,
} from './types';

// Identifier types - a single export statement exports both type and value
// when they share the same name (newtype pattern)
export {
  UserId,
  GuildId,
  ChannelId,
  RoleId,
  EmojiId,
  StickerId,
  KeyIdentifier,
  TapRef,
  parseUserId,
  parseGuildId,
  parseChannelId,
  parseRoleId,
  parseEmojiId,
  parseStickerId,
  parseKeyIdentifier,
  parseTapRef,
  getKeyTab,
  getKeyCategory,
  getKeyName,
  composeKeyIdentifier,
} from './types';

// Result type
export {
  type Result,
  type Ok,
  type Err,
  ok,
  err,
  isOk,
  isErr,
  unwrap,
  unwrapOr,
  unwrapOrElse,
  unwrapErr,
  map,
  mapErr,
  flatMap,
  andThen,
  all,
  any,
  fromPromise,
  toPromise,
  match,
} from './types';

// Primitive value types
export {
  type ValueTypeDescriptor,
  type BooleanTypeDescriptor,
  type IntegerTypeDescriptor,
  type IntegerRange,
  type StringTypeDescriptor,
  type StringPattern,
  type SomeOrDefault,
  type SomeOrDefaultTypeDescriptor,
  type ListTypeDescriptor,
  type AnyValueTypeDescriptor,
  booleanType,
  integerType,
  stringType,
  someOrDefaultType,
  listType,
  useDefault,
  useSome,
  isDefault,
  isSome,
  unwrapOrDefault,
} from './types';

// Special value types
export {
  type EnumDescribable,
  type VoiceChannelFollowingRule,
  type VoiceChannelFollowingRuleTypeDescriptor,
  vcFollowManual,
  vcFollowNonEmpty,
  voiceChannelFollowingRuleType,
  VOICE_CHANNEL_FOLLOWING_RULE_DESCRIPTIONS,
  type MemberFilter,
  type MemberFilterTypeDescriptor,
  type PermissionFlag,
  PermissionFlags,
  memberFilterAnyone,
  memberFilterWithPermission,
  memberFilterType,
  MEMBER_FILTER_DESCRIPTIONS,
  type TextMapping,
  type SimpleTextMapping,
  type RegexTextMapping,
  simpleTextMapping,
  regexTextMapping,
  type EmojiMapping,
  emojiMapping,
  type StickerMapping,
  stickerMapping,
  type MappingConfig,
  type MappingConfigTypeDescriptor,
  emptyMappingConfig,
  mappingConfig,
  mergeMappingConfigs,
  mappingConfigType,
  type TapRefTypeDescriptor,
  tapRefType,
  type AnySpecialValueTypeDescriptor,
} from './types';

// =============================================================================
// Scope
// =============================================================================

export {
  type SettingsKind,
  type UserScope,
  type UserScopeGlobal,
  type UserScopeGuild,
  type UserScopeUser,
  type UserScopePerGuildUser,
  type UserScopeId,
  type GuildScope,
  type GuildScopeGlobal,
  type GuildScopeGuild,
  type GuildScopeId,
  type AdminScope,
  type AdminScopeAdmin,
  type AdminScopeId,
  type Scope,
  type ScopeForKind,
  type SettingsActor,
  userScopeGlobal,
  userScopeGuild,
  userScopeUser,
  userScopePerGuildUser,
  guildScopeGlobal,
  guildScopeGuild,
  adminScopeAdmin,
  getScopeKind,
  isGlobalScope,
  getGuildIdFromScope,
  getUserIdFromScope,
  scopeToKey,
  actorAdmin,
  actorGuildAdmin,
  actorUser,
  canActorWriteToScope,
  USER_SCOPES_IN_ORDER,
  GUILD_SCOPES_IN_ORDER,
  ADMIN_SCOPES_IN_ORDER,
  USER_SCOPE_PRECEDENCE,
  GUILD_SCOPE_PRECEDENCE,
  getScopePrecedence,
  compareScopePrecedence,
  maxPrecedenceScope,
  minPrecedenceScope,
  type EntryPrecedenceInfo,
  compareEntryPrecedence,
  sortEntriesByPrecedence,
  findWinningEntry,
  filterByImportance,
  type ScopeChainContext,
  buildUserScopeChain,
  buildGuildScopeChain,
  buildAdminScopeChain,
  buildScopeChain,
} from './scope';

// =============================================================================
// Keys
// =============================================================================

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
  type KeyRegistryReader,
  type KeyRegistryWriter,
  type KeyRegistry,
  type KeyValueType,
  type KeySettingsKind,
  createKeyRegistry,
  getGlobalRegistry,
  resetGlobalRegistry,
  // Pre-defined keys
  USER_KEY_MAPPINGS,
  USER_KEY_READ_TEXT_NOT_IN_VC,
  USER_KEY_TTS_VOICE,
  USER_KEY_JOIN_ALERT_NAME,
  USER_KEY_STOP_TTS_KEYWORDS,
  USER_KEY_ALLOW_VOLUME_OVER_LIMIT,
  ALL_USER_KEYS,
  GUILD_KEY_VC_FOLLOWING_RULE,
  GUILD_KEY_JOIN_LEAVE_PERMISSION,
  GUILD_KEY_ENABLE_DISABLE_PERMISSION,
  GUILD_KEY_JOIN_WITHOUT_PERMISSION,
  ALL_GUILD_KEYS,
  ADMIN_KEY_ADMINS,
  ALL_ADMIN_KEYS,
} from './keys';

// =============================================================================
// Entry
// =============================================================================

export {
  type SettingsEntry,
  type UserSettingsEntry,
  type GuildSettingsEntry,
  type AdminSettingsEntry,
  type AnySettingsEntry,
  type StoredEntry,
  type StoredScope,
  createEntry,
  createUserEntry,
  createGuildEntry,
  createAdminEntry,
  withValue,
  withImportance,
  getEntryKind,
  serializeScope,
  deserializeScope,
  serializeEntry,
  deserializeEntry,
  type Mergeable,
  type MergeableTypeId,
  type MergeFunction,
  type IdentityFunction,
  registerMergeableType,
  getMergeFunction,
  getIdentityFunction,
  isMergeableType,
  mergeEntries,
  mergeEntriesWithDefault,
  isValueTypeMergeable,
} from './entry';

// =============================================================================
// Persistence
// =============================================================================

export {
  type EntryQuery,
  type ScopeQuery,
  type BatchEntryQuery,
  type IPersistenceAdapter,
  type PersistenceAdapterFactory,
  createInMemoryAdapter,
} from './persistence';

// =============================================================================
// Resolver
// =============================================================================

export {
  type UserResolutionContext,
  type GuildResolutionContext,
  type AdminResolutionContext,
  type ResolutionContext,
  userContext,
  guildContext,
  adminContext,
  getContextUserId,
  getContextGuildId,
  contextToScopeChainContext,
  type ResolvedValue,
  type ResolvedValueSource,
  type ISettingsResolver,
  createResolver,
} from './resolver';

// =============================================================================
// Cache
// =============================================================================

export { type ISettingsCache } from './cache';
export { createMemoryCache, type MemoryCacheConfig } from './cache/memory';

// =============================================================================
// Manager
// =============================================================================

export {
  type SettingsManagerConfig,
  type ISettingsManager,
  createSettingsManager,
} from './manager';
