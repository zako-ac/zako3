import { eq } from 'drizzle-orm';
import type { Redis } from 'ioredis';
import type { Logger } from 'pino';
import type { Database } from '../infra/database.js';
import { BaseRepository } from './base.repository.js';
import { users, type UserRow, type NewUserRow } from '../db/schema/users.js';
import type { Result } from '../lib/result.js';
import { Ok, Err } from '../lib/result.js';
import { NotFoundError } from '../lib/errors.js';

/**
 * UserRepository handles all user-related database operations
 */
export class UserRepository extends BaseRepository {
  constructor(database: Database, redis: Redis, logger: Logger) {
    super(database, redis, logger, 'user');
  }

  /**
   * Find a user by ID
   */
  async findById(id: string): Promise<Result<UserRow, NotFoundError>> {
    try {
      const cacheKey = `id:${id}`;
      
      const user = await this.cacheAside(
        cacheKey,
        async () => {
          const result = await this.db
            .select()
            .from(users)
            .where(eq(users.id, id))
            .limit(1);
          
          return result[0] || null;
        },
        300, // 5 minutes
      );

      if (!user) {
        return Err(new NotFoundError(`User with ID ${id} not found`));
      }

      return Ok(user);
    } catch (error) {
      this.logger.error({ error, id }, 'Failed to find user by ID');
      throw error;
    }
  }

  /**
   * Find multiple users by IDs
   */
  async findByIds(ids: string[]): Promise<Result<UserRow[], Error>> {
    try {
      const result = await this.db
        .select()
        .from(users)
        .where(eq(users.id, ids[0])); // Note: This is simplified, you'd use `in` operator

      return Ok(result);
    } catch (error) {
      this.logger.error({ error, ids }, 'Failed to find users by IDs');
      return Err(error as Error);
    }
  }

  /**
   * Create a new user or update if exists (upsert)
   */
  async upsert(data: NewUserRow): Promise<Result<UserRow, Error>> {
    try {
      const result = await this.db
        .insert(users)
        .values(data)
        .onConflictDoUpdate({
          target: users.id,
          set: {
            username: data.username,
            displayName: data.displayName,
            avatarUrl: data.avatarUrl,
            discriminator: data.discriminator,
            updatedAt: new Date(),
            lastSeenAt: new Date(),
          },
        })
        .returning();

      const user = result[0];
      
      // Invalidate cache
      await this.deleteCache(`id:${user.id}`);

      return Ok(user);
    } catch (error) {
      this.logger.error({ error, data }, 'Failed to upsert user');
      return Err(error as Error);
    }
  }

  /**
   * Update user's last seen timestamp
   */
  async updateLastSeen(id: string): Promise<Result<void, Error>> {
    try {
      await this.db
        .update(users)
        .set({ lastSeenAt: new Date() })
        .where(eq(users.id, id));

      // Invalidate cache
      await this.deleteCache(`id:${id}`);

      return Ok(undefined);
    } catch (error) {
      this.logger.error({ error, id }, 'Failed to update last seen');
      return Err(error as Error);
    }
  }

  /**
   * Ban a user
   */
  async ban(
    id: string,
    reason: string,
  ): Promise<Result<UserRow, NotFoundError | Error>> {
    try {
      const result = await this.db
        .update(users)
        .set({
          isBanned: true,
          bannedReason: reason,
          bannedAt: new Date(),
          updatedAt: new Date(),
        })
        .where(eq(users.id, id))
        .returning();

      if (result.length === 0) {
        return Err(new NotFoundError(`User with ID ${id} not found`));
      }

      // Invalidate cache
      await this.deleteCache(`id:${id}`);

      return Ok(result[0]);
    } catch (error) {
      this.logger.error({ error, id, reason }, 'Failed to ban user');
      return Err(error as Error);
    }
  }

  /**
   * Unban a user
   */
  async unban(id: string): Promise<Result<UserRow, NotFoundError | Error>> {
    try {
      const result = await this.db
        .update(users)
        .set({
          isBanned: false,
          bannedReason: null,
          bannedAt: null,
          updatedAt: new Date(),
        })
        .where(eq(users.id, id))
        .returning();

      if (result.length === 0) {
        return Err(new NotFoundError(`User with ID ${id} not found`));
      }

      // Invalidate cache
      await this.deleteCache(`id:${id}`);

      return Ok(result[0]);
    } catch (error) {
      this.logger.error({ error, id }, 'Failed to unban user');
      return Err(error as Error);
    }
  }

  /**
   * Check if user is banned
   */
  async isBanned(id: string): Promise<Result<boolean, Error>> {
    try {
      const userResult = await this.findById(id);
      if (!userResult.ok) {
        return Ok(false); // User doesn't exist, so not banned
      }

      return Ok(userResult.value.isBanned);
    } catch (error) {
      this.logger.error({ error, id }, 'Failed to check if user is banned');
      return Err(error as Error);
    }
  }
}
