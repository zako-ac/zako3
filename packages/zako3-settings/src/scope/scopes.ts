/**
 * @fileoverview Scope definitions for the settings system.
 *
 * Scopes define the context in which settings apply. Each settings kind
 * (User, Guild, Admin) has its own set of scopes with different precedences.
 */

import type { UserId, GuildId } from '../types';

// =============================================================================
// Settings Kind
// =============================================================================

/**
 * The three kinds of settings in the system.
 */
export type SettingsKind = 'user' | 'guild' | 'admin';

// =============================================================================
// User Scopes
// =============================================================================

/**
 * Global scope - derived from default values, not stored in DB.
 * This is a meta-scope representing the system-wide defaults.
 */
export interface UserScopeGlobal {
  readonly kind: 'user';
  readonly scope: 'global';
}

/**
 * Guild scope - settings that apply to all users in a guild.
 */
export interface UserScopeGuild {
  readonly kind: 'user';
  readonly scope: 'guild';
  readonly guildId: GuildId;
}

/**
 * User scope - settings specific to a user across all guilds.
 */
export interface UserScopeUser {
  readonly kind: 'user';
  readonly scope: 'user';
  readonly userId: UserId;
}

/**
 * Per-guild user scope - settings for a specific user in a specific guild.
 */
export interface UserScopePerGuildUser {
  readonly kind: 'user';
  readonly scope: 'perGuildUser';
  readonly guildId: GuildId;
  readonly userId: UserId;
}

/**
 * Union of all user setting scopes.
 */
export type UserScope =
  | UserScopeGlobal
  | UserScopeGuild
  | UserScopeUser
  | UserScopePerGuildUser;

/**
 * User scope identifiers (without the IDs).
 */
export type UserScopeId = UserScope['scope'];

// =============================================================================
// Guild Scopes
// =============================================================================

/**
 * Global scope for guild settings - derived from default values.
 */
export interface GuildScopeGlobal {
  readonly kind: 'guild';
  readonly scope: 'global';
}

/**
 * Guild scope - settings specific to a guild.
 */
export interface GuildScopeGuild {
  readonly kind: 'guild';
  readonly scope: 'guild';
  readonly guildId: GuildId;
}

/**
 * Union of all guild setting scopes.
 */
export type GuildScope = GuildScopeGlobal | GuildScopeGuild;

/**
 * Guild scope identifiers (without the IDs).
 */
export type GuildScopeId = GuildScope['scope'];

// =============================================================================
// Admin Scopes
// =============================================================================

/**
 * Admin scope - the only scope for admin settings.
 */
export interface AdminScopeAdmin {
  readonly kind: 'admin';
  readonly scope: 'admin';
}

/**
 * Union of all admin setting scopes (just one).
 */
export type AdminScope = AdminScopeAdmin;

/**
 * Admin scope identifiers.
 */
export type AdminScopeId = AdminScope['scope'];

// =============================================================================
// Combined Scope Types
// =============================================================================

/**
 * Union of all possible scopes across all settings kinds.
 */
export type Scope = UserScope | GuildScope | AdminScope;

/**
 * Maps a settings kind to its scope type.
 */
export type ScopeForKind<K extends SettingsKind> = K extends 'user'
  ? UserScope
  : K extends 'guild'
  ? GuildScope
  : K extends 'admin'
  ? AdminScope
  : never;

// =============================================================================
// Scope Constructors
// =============================================================================

/**
 * Creates a global user scope.
 */
export function userScopeGlobal(): UserScopeGlobal {
  return { kind: 'user', scope: 'global' };
}

/**
 * Creates a guild-level user scope.
 */
export function userScopeGuild(guildId: GuildId): UserScopeGuild {
  return { kind: 'user', scope: 'guild', guildId };
}

/**
 * Creates a user-level user scope.
 */
export function userScopeUser(userId: UserId): UserScopeUser {
  return { kind: 'user', scope: 'user', userId };
}

/**
 * Creates a per-guild-user scope.
 */
export function userScopePerGuildUser(guildId: GuildId, userId: UserId): UserScopePerGuildUser {
  return { kind: 'user', scope: 'perGuildUser', guildId, userId };
}

/**
 * Creates a global guild scope.
 */
export function guildScopeGlobal(): GuildScopeGlobal {
  return { kind: 'guild', scope: 'global' };
}

/**
 * Creates a guild-level guild scope.
 */
export function guildScopeGuild(guildId: GuildId): GuildScopeGuild {
  return { kind: 'guild', scope: 'guild', guildId };
}

/**
 * Creates an admin scope.
 */
export function adminScopeAdmin(): AdminScopeAdmin {
  return { kind: 'admin', scope: 'admin' };
}

// =============================================================================
// Scope Utilities
// =============================================================================

/**
 * Gets the settings kind from a scope.
 */
export function getScopeKind(scope: Scope): SettingsKind {
  return scope.kind;
}

/**
 * Checks if a scope is a global (meta) scope.
 */
export function isGlobalScope(scope: Scope): boolean {
  return scope.scope === 'global';
}

/**
 * Extracts the guild ID from a scope, if present.
 */
export function getGuildIdFromScope(scope: Scope): GuildId | null {
  if ('guildId' in scope) {
    return scope.guildId;
  }
  return null;
}

/**
 * Extracts the user ID from a scope, if present.
 */
export function getUserIdFromScope(scope: Scope): UserId | null {
  if ('userId' in scope) {
    return scope.userId;
  }
  return null;
}

/**
 * Creates a unique string key for a scope (useful for caching/maps).
 */
export function scopeToKey(scope: Scope): string {
  switch (scope.kind) {
    case 'user':
      switch (scope.scope) {
        case 'global':
          return 'user:global';
        case 'guild':
          return `user:guild:${scope.guildId}`;
        case 'user':
          return `user:user:${scope.userId}`;
        case 'perGuildUser':
          return `user:perGuildUser:${scope.guildId}:${scope.userId}`;
      }
      break;
    case 'guild':
      switch (scope.scope) {
        case 'global':
          return 'guild:global';
        case 'guild':
          return `guild:guild:${scope.guildId}`;
      }
      break;
    case 'admin':
      return 'admin:admin';
  }
}

// =============================================================================
// Writable-by Permissions
// =============================================================================

/**
 * Actors who can write to settings.
 */
export type SettingsActor =
  | { readonly kind: 'admin' }
  | { readonly kind: 'guildAdmin'; readonly guildId: GuildId }
  | { readonly kind: 'user'; readonly userId: UserId };

/**
 * Creates an admin actor.
 */
export function actorAdmin(): SettingsActor {
  return { kind: 'admin' };
}

/**
 * Creates a guild admin actor.
 */
export function actorGuildAdmin(guildId: GuildId): SettingsActor {
  return { kind: 'guildAdmin', guildId };
}

/**
 * Creates a user actor.
 */
export function actorUser(userId: UserId): SettingsActor {
  return { kind: 'user', userId };
}

/**
 * Checks if an actor can write to a specific scope.
 *
 * Permission rules:
 * - Admin can write to any scope
 * - Guild Admin can write to their guild's scopes
 * - User can write to their own user scopes
 *
 * @param actor - The actor attempting to write
 * @param scope - The scope being written to
 * @returns True if the actor can write to the scope
 */
export function canActorWriteToScope(actor: SettingsActor, scope: Scope): boolean {
  // Admin can write anywhere
  if (actor.kind === 'admin') {
    return true;
  }

  // Global scopes are admin-only
  if (isGlobalScope(scope)) {
    return false;
  }

  switch (scope.kind) {
    case 'user': {
      switch (scope.scope) {
        case 'global':
          return false; // Already handled above
        case 'guild':
          // Only admin + guild admin can write to guild-level user settings
          return (
            actor.kind === 'guildAdmin' && actor.guildId === scope.guildId
          );
        case 'user':
          // Admin + the user themselves can write
          return actor.kind === 'user' && actor.userId === scope.userId;
        case 'perGuildUser':
          // Admin + User + Guild Admin can write
          if (actor.kind === 'user' && actor.userId === scope.userId) {
            return true;
          }
          if (actor.kind === 'guildAdmin' && actor.guildId === scope.guildId) {
            return true;
          }
          return false;
      }
      break;
    }
    case 'guild': {
      switch (scope.scope) {
        case 'global':
          return false; // Already handled above
        case 'guild':
          // Admin + guild admin can write
          return (
            actor.kind === 'guildAdmin' && actor.guildId === scope.guildId
          );
      }
      break;
    }
    case 'admin':
      // Only admin can write to admin settings
      return false;
  }
}

// =============================================================================
// All Scope Lists (for iteration)
// =============================================================================

/**
 * All user scope identifiers in precedence order (ascending).
 */
export const USER_SCOPES_IN_ORDER: readonly UserScopeId[] = [
  'global',
  'guild',
  'user',
  'perGuildUser',
] as const;

/**
 * All guild scope identifiers in precedence order (ascending).
 */
export const GUILD_SCOPES_IN_ORDER: readonly GuildScopeId[] = [
  'global',
  'guild',
] as const;

/**
 * All admin scope identifiers.
 */
export const ADMIN_SCOPES_IN_ORDER: readonly AdminScopeId[] = ['admin'] as const;
