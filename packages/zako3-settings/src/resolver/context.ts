/**
 * @fileoverview Resolution context for settings queries.
 *
 * The resolution context provides the information needed to determine
 * which scopes to query and how to resolve settings values.
 */

import type { UserId, GuildId } from '../types';

// =============================================================================
// Resolution Context
// =============================================================================

/**
 * Context for resolving user settings.
 * Determines which scopes are applicable for the resolution.
 */
export interface UserResolutionContext {
  /** The user requesting the setting */
  readonly userId: UserId;

  /** The guild context (if in a guild) */
  readonly guildId?: GuildId;
}

/**
 * Context for resolving guild settings.
 */
export interface GuildResolutionContext {
  /** The guild to resolve settings for */
  readonly guildId: GuildId;
}

/**
 * Context for resolving admin settings.
 * Admin settings have no context - they're global.
 */
export interface AdminResolutionContext {
  // Empty - admin settings are global
}

/**
 * Union of all resolution contexts.
 */
export type ResolutionContext =
  | ({ readonly kind: 'user' } & UserResolutionContext)
  | ({ readonly kind: 'guild' } & GuildResolutionContext)
  | ({ readonly kind: 'admin' } & AdminResolutionContext);

// =============================================================================
// Context Constructors
// =============================================================================

/**
 * Creates a user resolution context.
 *
 * @param userId - The user ID
 * @param guildId - Optional guild ID
 * @returns A user resolution context
 */
export function userContext(userId: UserId, guildId?: GuildId): ResolutionContext {
  return {
    kind: 'user',
    userId,
    guildId,
  };
}

/**
 * Creates a guild resolution context.
 *
 * @param guildId - The guild ID
 * @returns A guild resolution context
 */
export function guildContext(guildId: GuildId): ResolutionContext {
  return {
    kind: 'guild',
    guildId,
  };
}

/**
 * Creates an admin resolution context.
 *
 * @returns An admin resolution context
 */
export function adminContext(): ResolutionContext {
  return {
    kind: 'admin',
  };
}

// =============================================================================
// Context Utilities
// =============================================================================

/**
 * Extracts user ID from context if available.
 */
export function getContextUserId(context: ResolutionContext): UserId | undefined {
  if (context.kind === 'user') {
    return context.userId;
  }
  return undefined;
}

/**
 * Extracts guild ID from context if available.
 */
export function getContextGuildId(context: ResolutionContext): GuildId | undefined {
  if (context.kind === 'user' && context.guildId) {
    return context.guildId;
  }
  if (context.kind === 'guild') {
    return context.guildId;
  }
  return undefined;
}

/**
 * Converts a resolution context to a scope chain context.
 */
export function contextToScopeChainContext(context: ResolutionContext): {
  guildId?: string;
  userId?: string;
} {
  return {
    guildId: getContextGuildId(context) as string | undefined,
    userId: getContextUserId(context) as string | undefined,
  };
}
