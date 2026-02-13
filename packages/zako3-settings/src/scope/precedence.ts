/**
 * @fileoverview Scope precedence and ordering logic.
 *
 * This module handles the complex precedence rules for settings resolution,
 * including support for "important" entries that invert normal precedence.
 */

import type {
  Scope,
  UserScope,
  GuildScope,
  AdminScope,
  UserScopeId,
  GuildScopeId,
  SettingsKind,
} from './scopes';

// =============================================================================
// Precedence Values
// =============================================================================

/**
 * Numeric precedence values for user scopes.
 * Higher values = higher precedence (wins in conflicts).
 */
export const USER_SCOPE_PRECEDENCE: Readonly<Record<UserScopeId, number>> = {
  global: 0,
  guild: 1,
  user: 2,
  perGuildUser: 3,
};

/**
 * Numeric precedence values for guild scopes.
 */
export const GUILD_SCOPE_PRECEDENCE: Readonly<Record<GuildScopeId, number>> = {
  global: 0,
  guild: 1,
};

/**
 * Gets the precedence value for a scope.
 *
 * @param scope - The scope to get precedence for
 * @returns Numeric precedence value
 */
export function getScopePrecedence(scope: Scope): number {
  switch (scope.kind) {
    case 'user':
      return USER_SCOPE_PRECEDENCE[scope.scope];
    case 'guild':
      return GUILD_SCOPE_PRECEDENCE[scope.scope];
    case 'admin':
      return 0; // Admin has only one scope
  }
}

// =============================================================================
// Scope Comparison
// =============================================================================

/**
 * Compares two scopes by precedence.
 *
 * @param a - First scope
 * @param b - Second scope
 * @returns Negative if a < b, positive if a > b, zero if equal
 * @throws Error if scopes are from different kinds
 */
export function compareScopePrecedence(a: Scope, b: Scope): number {
  if (a.kind !== b.kind) {
    throw new Error(`Cannot compare scopes of different kinds: ${a.kind} vs ${b.kind}`);
  }
  return getScopePrecedence(a) - getScopePrecedence(b);
}

/**
 * Returns the higher precedence scope.
 *
 * @param a - First scope
 * @param b - Second scope
 * @returns The scope with higher precedence
 */
export function maxPrecedenceScope<S extends Scope>(a: S, b: S): S {
  return compareScopePrecedence(a, b) >= 0 ? a : b;
}

/**
 * Returns the lower precedence scope.
 *
 * @param a - First scope
 * @param b - Second scope
 * @returns The scope with lower precedence
 */
export function minPrecedenceScope<S extends Scope>(a: S, b: S): S {
  return compareScopePrecedence(a, b) <= 0 ? a : b;
}

// =============================================================================
// Important Entry Precedence
// =============================================================================

/**
 * Entry metadata for precedence calculation.
 */
export interface EntryPrecedenceInfo {
  readonly scope: Scope;
  readonly isImportant: boolean;
}

/**
 * Compares two entries considering the "important" flag.
 *
 * Important entries invert the normal precedence order:
 * - Non-important entries are compared normally (higher scope wins)
 * - Important entries take precedence over non-important ones
 * - Among important entries, LOWER scope wins (inverted)
 *
 * @param a - First entry info
 * @param b - Second entry info
 * @returns Negative if a wins, positive if b wins, zero if equal
 */
export function compareEntryPrecedence(
  a: EntryPrecedenceInfo,
  b: EntryPrecedenceInfo
): number {
  // If both have same importance, compare normally or inverted
  if (a.isImportant === b.isImportant) {
    const scopeComparison = compareScopePrecedence(a.scope, b.scope);
    // Important entries: lower scope wins (invert comparison)
    // Non-important: higher scope wins (normal comparison)
    return a.isImportant ? -scopeComparison : scopeComparison;
  }

  // Important beats non-important
  return a.isImportant ? 1 : -1;
}

/**
 * Sorts entries by precedence (highest precedence first).
 *
 * @param entries - Array of entry precedence info
 * @returns New array sorted by precedence (descending)
 */
export function sortEntriesByPrecedence<T extends EntryPrecedenceInfo>(
  entries: readonly T[]
): T[] {
  return [...entries].sort((a, b) => -compareEntryPrecedence(a, b));
}

/**
 * Finds the winning entry among a list of entries.
 *
 * @param entries - Array of entry precedence info
 * @returns The entry with highest precedence, or undefined if empty
 */
export function findWinningEntry<T extends EntryPrecedenceInfo>(
  entries: readonly T[]
): T | undefined {
  if (entries.length === 0) {
    return undefined;
  }

  let winner = entries[0];
  for (let i = 1; i < entries.length; i++) {
    if (compareEntryPrecedence(entries[i], winner) > 0) {
      winner = entries[i];
    }
  }
  return winner;
}

// =============================================================================
// Mixed Important/Non-Important Handling
// =============================================================================

/**
 * Filters entries based on the "all non-important ignored if any important exists" rule.
 *
 * From the spec: "If specific key has mixed important/non-important entries per scope,
 * all non-important entries are ignored."
 *
 * @param entries - Array of entries
 * @returns Filtered array (only important entries if any exist, otherwise all)
 */
export function filterByImportance<T extends EntryPrecedenceInfo>(
  entries: readonly T[]
): T[] {
  const hasImportant = entries.some((e) => e.isImportant);
  if (!hasImportant) {
    return [...entries];
  }
  return entries.filter((e) => e.isImportant);
}

// =============================================================================
// Scope Chain Building
// =============================================================================

/**
 * Context for building a scope chain (the scopes to query for a resolution).
 */
export interface ScopeChainContext {
  readonly guildId?: string;
  readonly userId?: string;
}

/**
 * Builds the ordered list of user scopes to check for a given context.
 * Order is from lowest to highest precedence.
 *
 * @param context - The resolution context
 * @returns Array of user scopes in precedence order
 */
export function buildUserScopeChain(
  context: ScopeChainContext
): UserScope[] {
  const chain: UserScope[] = [];

  // Global is always included (but resolved from defaults, not DB)
  chain.push({ kind: 'user', scope: 'global' });

  // Guild scope if guild context is provided
  if (context.guildId) {
    chain.push({
      kind: 'user',
      scope: 'guild',
      guildId: context.guildId as any, // Cast because we're building from string
    });
  }

  // User scope if user context is provided
  if (context.userId) {
    chain.push({
      kind: 'user',
      scope: 'user',
      userId: context.userId as any,
    });
  }

  // Per-guild-user scope if both are provided
  if (context.guildId && context.userId) {
    chain.push({
      kind: 'user',
      scope: 'perGuildUser',
      guildId: context.guildId as any,
      userId: context.userId as any,
    });
  }

  return chain;
}

/**
 * Builds the ordered list of guild scopes to check for a given context.
 *
 * @param context - The resolution context (must have guildId)
 * @returns Array of guild scopes in precedence order
 */
export function buildGuildScopeChain(
  context: ScopeChainContext
): GuildScope[] {
  const chain: GuildScope[] = [];

  // Global is always included
  chain.push({ kind: 'guild', scope: 'global' });

  // Guild scope if context is provided
  if (context.guildId) {
    chain.push({
      kind: 'guild',
      scope: 'guild',
      guildId: context.guildId as any,
    });
  }

  return chain;
}

/**
 * Builds the scope chain for admin settings.
 * Admin settings only have one scope.
 *
 * @returns Array with single admin scope
 */
export function buildAdminScopeChain(): AdminScope[] {
  return [{ kind: 'admin', scope: 'admin' }];
}

/**
 * Builds the appropriate scope chain based on settings kind.
 *
 * @param kind - The settings kind
 * @param context - The resolution context
 * @returns Array of scopes in precedence order
 */
export function buildScopeChain(
  kind: SettingsKind,
  context: ScopeChainContext
): Scope[] {
  switch (kind) {
    case 'user':
      return buildUserScopeChain(context);
    case 'guild':
      return buildGuildScopeChain(context);
    case 'admin':
      return buildAdminScopeChain();
  }
}
