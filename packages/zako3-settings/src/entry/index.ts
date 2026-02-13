/**
 * @fileoverview Entry module exports.
 */

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
} from './entry';

export {
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
} from './merging';
