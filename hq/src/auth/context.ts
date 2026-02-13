import { type UserId, type GuildId } from 'zako3-settings';

export interface JWTPayload {
  sub: string;
  iat: number;
  exp: number;
}

export interface AuthContext {
  userId: UserId;
  raw: JWTPayload;
}

export interface IGuildPermissionChecker {
  isGuildAdmin(userId: UserId, guildId: GuildId): Promise<boolean>;
}

export interface IBotAdminChecker {
  isBotAdmin(userId: UserId): Promise<boolean>;
}

export interface IAuthorizationService {
  guildChecker: IGuildPermissionChecker;
  adminChecker: IBotAdminChecker;
}

export type AuthorizationResult =
  | { authorized: true }
  | { authorized: false; reason: string };

export const AuthErrors = {
  MISSING_TOKEN: 'Missing authorization token',
  INVALID_TOKEN: 'Invalid authorization token',
  TOKEN_EXPIRED: 'Token has expired',
  JWKS_FETCH_FAILED: 'Failed to fetch JWKS',
  FORBIDDEN: 'Access denied',
  NOT_GUILD_ADMIN: 'User is not a guild admin',
  NOT_BOT_ADMIN: 'User is not a bot admin',
  NOT_OWNER: 'User is not the owner of this resource',
} as const;

export type AuthErrorCode = keyof typeof AuthErrors;
