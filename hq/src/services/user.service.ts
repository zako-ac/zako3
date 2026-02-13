import type { Logger } from 'pino';
import { UserRepository } from '../repositories/user.repository.js';
import { type Result, Ok, Err, isOk } from '../lib/result.js';
import { NotFoundError, ForbiddenError } from '../lib/errors.js';
import type { UserRow, NewUserRow } from '../db/schema/users.js';

/**
 * User type for API responses
 */
export interface User {
  id: string;
  username: string;
  displayName?: string;
  avatarUrl?: string;
  discriminator?: string;
  isBanned: boolean;
  createdAt: string;
  lastSeenAt?: string;
}

/**
 * Convert database UserRow to API User type
 */
function mapUserRowToUser(row: UserRow): User {
  return {
    id: row.id,
    username: row.username,
    displayName: row.displayName || undefined,
    avatarUrl: row.avatarUrl || undefined,
    discriminator: row.discriminator || undefined,
    isBanned: row.isBanned,
    createdAt: row.createdAt.toISOString(),
    lastSeenAt: row.lastSeenAt?.toISOString(),
  };
}

/**
 * UserService provides business logic for user operations
 * Transport-agnostic: can be called from REST API or Discord bot
 */
export class UserService {
  constructor(
    private userRepo: UserRepository,
    private logger: Logger,
  ) {
    this.logger = logger.child({ service: 'UserService' });
  }

  /**
   * Get a user by ID
   */
  async getUser(userId: string): Promise<Result<User, NotFoundError>> {
    this.logger.debug({ userId }, 'Getting user');

    const result = await this.userRepo.findById(userId);
    if (!isOk(result)) {
      return result;
    }

    return Ok(mapUserRowToUser(result.value));
  }

  /**
   * Get a public user profile (filtered fields)
   */
  async getPublicProfile(userId: string): Promise<Result<User, NotFoundError>> {
    // For now, same as getUser. In the future, might filter sensitive fields
    return this.getUser(userId);
  }

  /**
   * Create or update a user from Discord data
   */
  async upsertFromDiscord(discordUser: {
    id: string;
    username: string;
    displayName?: string;
    avatarUrl?: string;
    discriminator?: string;
  }): Promise<Result<User, Error>> {
    this.logger.debug({ discordUser }, 'Upserting user from Discord');

    const userData: NewUserRow = {
      id: discordUser.id,
      username: discordUser.username,
      displayName: discordUser.displayName || null,
      avatarUrl: discordUser.avatarUrl || null,
      discriminator: discordUser.discriminator || null,
      lastSeenAt: new Date(),
    };

    const result = await this.userRepo.upsert(userData);
    if (!isOk(result)) {
      return result;
    }

    return Ok(mapUserRowToUser(result.value));
  }

  /**
   * Update user's last seen timestamp
   */
  async updateLastSeen(userId: string): Promise<Result<void, Error>> {
    this.logger.debug({ userId }, 'Updating last seen');
    return this.userRepo.updateLastSeen(userId);
  }

  /**
   * Ban a user (admin only)
   */
  async banUser(
    userId: string,
    reason: string,
    adminUserId: string,
  ): Promise<Result<User, NotFoundError | Error>> {
    this.logger.info({ userId, adminUserId, reason }, 'Banning user');

    // Note: Permission check should be done by the caller (API layer or bot layer)
    // This service is transport-agnostic

    const result = await this.userRepo.ban(userId, reason);
    if (!isOk(result)) {
      return result;
    }

    return Ok(mapUserRowToUser(result.value));
  }

  /**
   * Unban a user (admin only)
   */
  async unbanUser(
    userId: string,
    adminUserId: string,
  ): Promise<Result<User, NotFoundError | Error>> {
    this.logger.info({ userId, adminUserId }, 'Unbanning user');

    const result = await this.userRepo.unban(userId);
    if (!isOk(result)) {
      return result;
    }

    return Ok(mapUserRowToUser(result.value));
  }

  /**
   * Check if a user is banned
   */
  async isUserBanned(userId: string): Promise<Result<boolean, Error>> {
    this.logger.debug({ userId }, 'Checking if user is banned');
    return this.userRepo.isBanned(userId);
  }

  /**
   * Verify user is not banned, throws if banned
   */
  async ensureNotBanned(userId: string): Promise<Result<void, ForbiddenError | Error>> {
    const result = await this.userRepo.isBanned(userId);
    if (!isOk(result)) {
      return result;
    }

    if (result.value) {
      return Err(new ForbiddenError('User is banned'));
    }

    return Ok(undefined);
  }
}
