import { eq, and, sql, desc, or, ilike, inArray } from 'drizzle-orm';
import type { Redis } from 'ioredis';
import type { Logger } from 'pino';
import type { Database } from '../infra/database.js';
import { BaseRepository } from './base.repository.js';
import {
  taps,
  tapMembers,
  type TapRow,
  type NewTapRow,
  type TapMemberRow,
  type NewTapMemberRow,
} from '../db/schema/taps.js';
import type { Result } from '../lib/result.js';
import { Ok, Err } from '../lib/result.js';
import { NotFoundError, ConflictError } from '../lib/errors.js';
import {
  createPaginatedResponse,
  normalizePaginationParams,
  calculateOffset,
  type PaginatedResponse,
} from '../lib/pagination.js';

/**
 * TapRepository handles all tap-related database operations
 */
export class TapRepository extends BaseRepository {
  constructor(database: Database, redis: Redis, logger: Logger) {
    super(database, redis, logger, 'tap');
  }

  /**
   * Find a tap by ID
   */
  async findById(id: string): Promise<Result<TapRow, NotFoundError>> {
    try {
      const cacheKey = `id:${id}`;

      const tap = await this.cacheAside(
        cacheKey,
        async () => {
          const result = await this.db
            .select()
            .from(taps)
            .where(eq(taps.id, id))
            .limit(1);

          return result[0] || null;
        },
        300, // 5 minutes
      );

      if (!tap) {
        return Err(new NotFoundError(`Tap with ID ${id} not found`));
      }

      return Ok(tap);
    } catch (error) {
      this.logger.error({ error, id }, 'Failed to find tap by ID');
      throw error;
    }
  }

  /**
   * Create a new tap
   */
  async create(data: NewTapRow): Promise<Result<TapRow, ConflictError | Error>> {
    try {
      // Check if tap ID already exists
      const existing = await this.findById(data.id);
      if (existing.ok) {
        return Err(new ConflictError(`Tap with ID ${data.id} already exists`));
      }

      const result = await this.db.insert(taps).values(data).returning();
      const tap = result[0];

      // Add owner as a member with 'owner' role
      await this.db.insert(tapMembers).values({
        tapId: tap.id,
        userId: tap.ownerId,
        role: 'owner',
      });

      return Ok(tap);
    } catch (error) {
      this.logger.error({ error, data }, 'Failed to create tap');
      return Err(error as Error);
    }
  }

  /**
   * Update a tap
   */
  async update(
    id: string,
    data: Partial<Omit<TapRow, 'id' | 'createdAt' | 'updatedAt'>>,
  ): Promise<Result<TapRow, NotFoundError | Error>> {
    try {
      const result = await this.db
        .update(taps)
        .set({ ...data, updatedAt: new Date() })
        .where(eq(taps.id, id))
        .returning();

      if (result.length === 0) {
        return Err(new NotFoundError(`Tap with ID ${id} not found`));
      }

      // Invalidate cache
      await this.deleteCache(`id:${id}`);

      return Ok(result[0]);
    } catch (error) {
      this.logger.error({ error, id, data }, 'Failed to update tap');
      return Err(error as Error);
    }
  }

  /**
   * Delete a tap
   */
  async delete(id: string): Promise<Result<void, NotFoundError | Error>> {
    try {
      const result = await this.db.delete(taps).where(eq(taps.id, id)).returning();

      if (result.length === 0) {
        return Err(new NotFoundError(`Tap with ID ${id} not found`));
      }

      // Invalidate cache
      await this.deleteCache(`id:${id}`);
      await this.deleteCachePattern(`members:${id}:*`);

      return Ok(undefined);
    } catch (error) {
      this.logger.error({ error, id }, 'Failed to delete tap');
      return Err(error as Error);
    }
  }

  /**
   * List taps with pagination and filtering
   */
  async list(options: {
    page?: number;
    limit?: number;
    search?: string;
    isVerified?: boolean;
    ownerId?: string;
  }): Promise<Result<PaginatedResponse<TapRow>, Error>> {
    try {
      const { page, limit } = normalizePaginationParams(options.page, options.limit);
      const offset = calculateOffset(page, limit);

      // Build where conditions
      const conditions = [];
      if (options.search) {
        conditions.push(
          or(
            ilike(taps.name, `%${options.search}%`),
            ilike(taps.description, `%${options.search}%`),
          ),
        );
      }
      if (options.isVerified !== undefined) {
        conditions.push(eq(taps.isVerified, options.isVerified));
      }
      if (options.ownerId) {
        conditions.push(eq(taps.ownerId, options.ownerId));
      }

      const whereClause = conditions.length > 0 ? and(...conditions) : undefined;

      // Get total count
      const countResult = await this.db
        .select({ count: sql<number>`count(*)::int` })
        .from(taps)
        .where(whereClause);
      const total = countResult[0]?.count || 0;

      // Get paginated data
      const data = await this.db
        .select()
        .from(taps)
        .where(whereClause)
        .orderBy(desc(taps.createdAt))
        .limit(limit)
        .offset(offset);

      return Ok(createPaginatedResponse(data, page, limit, total));
    } catch (error) {
      this.logger.error({ error, options }, 'Failed to list taps');
      return Err(error as Error);
    }
  }

  /**
   * Get tap members
   */
  async getMembers(tapId: string): Promise<Result<TapMemberRow[], Error>> {
    try {
      const cacheKey = `members:${tapId}:all`;

      const members = await this.cacheAside(
        cacheKey,
        async () => {
          return await this.db
            .select()
            .from(tapMembers)
            .where(eq(tapMembers.tapId, tapId));
        },
        300, // 5 minutes
      );

      return Ok(members);
    } catch (error) {
      this.logger.error({ error, tapId }, 'Failed to get tap members');
      return Err(error as Error);
    }
  }

  /**
   * Get user's role in a tap
   */
  async getUserRole(
    tapId: string,
    userId: string,
  ): Promise<Result<string | null, Error>> {
    try {
      const result = await this.db
        .select({ role: tapMembers.role })
        .from(tapMembers)
        .where(and(eq(tapMembers.tapId, tapId), eq(tapMembers.userId, userId)))
        .limit(1);

      return Ok(result[0]?.role || null);
    } catch (error) {
      this.logger.error({ error, tapId, userId }, 'Failed to get user role');
      return Err(error as Error);
    }
  }

  /**
   * Add a member to a tap
   */
  async addMember(
    data: NewTapMemberRow,
  ): Promise<Result<TapMemberRow, Error>> {
    try {
      const result = await this.db.insert(tapMembers).values(data).returning();

      // Update member count
      await this.db
        .update(taps)
        .set({ memberCount: sql`${taps.memberCount} + 1` })
        .where(eq(taps.id, data.tapId));

      // Invalidate caches
      await this.deleteCachePattern(`members:${data.tapId}:*`);
      await this.deleteCache(`id:${data.tapId}`);

      return Ok(result[0]);
    } catch (error) {
      this.logger.error({ error, data }, 'Failed to add tap member');
      return Err(error as Error);
    }
  }

  /**
   * Remove a member from a tap
   */
  async removeMember(
    tapId: string,
    userId: string,
  ): Promise<Result<void, Error>> {
    try {
      await this.db
        .delete(tapMembers)
        .where(and(eq(tapMembers.tapId, tapId), eq(tapMembers.userId, userId)));

      // Update member count
      await this.db
        .update(taps)
        .set({ memberCount: sql`${taps.memberCount} - 1` })
        .where(eq(taps.id, tapId));

      // Invalidate caches
      await this.deleteCachePattern(`members:${tapId}:*`);
      await this.deleteCache(`id:${tapId}`);

      return Ok(undefined);
    } catch (error) {
      this.logger.error({ error, tapId, userId }, 'Failed to remove tap member');
      return Err(error as Error);
    }
  }

  /**
   * Update member role
   */
  async updateMemberRole(
    tapId: string,
    userId: string,
    role: string,
  ): Promise<Result<TapMemberRow, Error>> {
    try {
      const result = await this.db
        .update(tapMembers)
        .set({ role })
        .where(and(eq(tapMembers.tapId, tapId), eq(tapMembers.userId, userId)))
        .returning();

      if (result.length === 0) {
        return Err(new NotFoundError('Member not found'));
      }

      // Invalidate caches
      await this.deleteCachePattern(`members:${tapId}:*`);

      return Ok(result[0]);
    } catch (error) {
      this.logger.error({ error, tapId, userId, role }, 'Failed to update member role');
      return Err(error as Error);
    }
  }

  /**
   * Check if user is a member of a tap
   */
  async isMember(tapId: string, userId: string): Promise<Result<boolean, Error>> {
    try {
      const result = await this.db
        .select({ userId: tapMembers.userId })
        .from(tapMembers)
        .where(and(eq(tapMembers.tapId, tapId), eq(tapMembers.userId, userId)))
        .limit(1);

      return Ok(result.length > 0);
    } catch (error) {
      this.logger.error({ error, tapId, userId }, 'Failed to check if user is member');
      return Err(error as Error);
    }
  }
}
