/**
 * @fileoverview Resolver module exports.
 */

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
} from './context';

export {
  type ResolvedValue,
  type ResolvedValueSource,
  type ISettingsResolver,
  createResolver,
} from './resolver';
