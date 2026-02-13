import type { Logger } from 'pino';
import { TapRepository } from '../repositories/tap.repository.js';
import { UserRepository } from '../repositories/user.repository.js';
import { type Result, Ok, Err, isOk } from '../lib/result.js';
import {
    NotFoundError,
    ForbiddenError,
    ConflictError,
    ValidationError,
} from '../lib/errors.js';
import type { TapRow, TapMemberRow, NewTapRow } from '../db/schema/taps.js';
import type { PaginatedResponse } from '../lib/pagination.js';

/**
 * Valid member roles
 */
const MEMBER_ROLES = ['owner', 'admin', 'moderator', 'member'] as const;

/**
 * Tap type for API responses
 */
export interface Tap {
  id: string;
  name: string;
  description?: string;
  ownerId: string;
  isPrivate: boolean;
  isLocked: boolean;
  isVerified: boolean;
  memberCount: number;
  createdAt: string;
  updatedAt: string;
}

/**
 * TapMember type for API responses
 */
export interface TapMember {
  tapId: string;
  userId: string;
  role: string;
  joinedAt: string;
}

/**
 * Create tap request
 */
export interface CreateTapRequest {
  id: string;
  name: string;
  description?: string;
  isPrivate?: boolean;
}

/**
 * Update tap request
 */
export interface UpdateTapRequest {
  name?: string;
  description?: string;
  isPrivate?: boolean;
  isLocked?: boolean;
}

/**
 * Convert database TapRow to API Tap type
 */
function mapTapRowToTap(row: TapRow): Tap {
    return {
        id: row.id,
        name: row.name,
        description: row.description || undefined,
        ownerId: row.ownerId,
        isPrivate: row.isPrivate,
        isLocked: row.isLocked,
        isVerified: row.isVerified,
        memberCount: row.memberCount,
        createdAt: row.createdAt.toISOString(),
        updatedAt: row.updatedAt.toISOString(),
    };
}

/**
 * Convert database TapMemberRow to API TapMember type
 */
function mapTapMemberRowToTapMember(row: TapMemberRow): TapMember {
    return {
        tapId: row.tapId,
        userId: row.userId,
        role: row.role,
        joinedAt: row.joinedAt.toISOString(),
    };
}

/**
 * Permission levels for tap operations
 */
const PERMISSION_LEVELS = {
    owner: 4,
    admin: 3,
    moderator: 2,
    member: 1,
} as const;

/**
 * TapService provides business logic for tap operations
 * Transport-agnostic: can be called from REST API or Discord bot
 */
export class TapService {
    constructor(
        private tapRepo: TapRepository,
        private userRepo: UserRepository,
        private logger: Logger,
    ) {
        this.logger = logger.child({ service: 'TapService' });
    }

    /**
     * Get a tap by ID
     */
    async getTap(
        tapId: string,
        requestingUserId?: string,
    ): Promise<Result<Tap, NotFoundError | ForbiddenError>> {
        this.logger.debug({ tapId, requestingUserId }, 'Getting tap');

        const result = await this.tapRepo.findById(tapId);
        if (!isOk(result)) {
            return result;
        }

        const tap = result.value;

        // Check if user has permission to view private tap
        if (tap.isPrivate && requestingUserId) {
            const isMemberResult = await this.tapRepo.isMember(tapId, requestingUserId);
            if (!isOk(isMemberResult)) {
                // Convert generic Error to ForbiddenError
                return Err(new ForbiddenError('Failed to check membership'));
            }

            if (!isMemberResult.value) {
                return Err(new ForbiddenError('This tap is private'));
            }
        }

        return Ok(mapTapRowToTap(tap));
    }

    /**
     * List taps with filtering and pagination
     */
    async listTaps(options: {
        page?: number;
        limit?: number;
        search?: string;
        isVerified?: boolean;
        ownerId?: string;
    }): Promise<Result<PaginatedResponse<Tap>, Error>> {
        this.logger.debug({ options }, 'Listing taps');

        const result = await this.tapRepo.list(options);
        if (!isOk(result)) {
            return result;
        }

        // Map rows to API types
        const mapped = {
            data: result.value.data.map(mapTapRowToTap),
            meta: result.value.meta,
        };

        return Ok(mapped);
    }

    /**
     * Create a new tap
     */
    async createTap(
        data: CreateTapRequest,
        userId: string,
    ): Promise<Result<Tap, ConflictError | ValidationError | Error>> {
        this.logger.info({ data, userId }, 'Creating tap');

        // Validate tap ID format (alphanumeric, hyphens, underscores, 3-50 chars)
        const tapIdRegex = /^[a-zA-Z0-9_-]{3,50}$/;
        if (!tapIdRegex.test(data.id)) {
            return Err(
                new ValidationError(
                    'Tap ID must be 3-50 characters and contain only letters, numbers, hyphens, and underscores',
                ),
            );
        }

        const tapData: NewTapRow = {
            id: data.id,
            name: data.name,
            description: data.description || null,
            ownerId: userId,
            isPrivate: data.isPrivate || false,
            isLocked: false,
            isVerified: false,
            memberCount: 1, // Owner is the first member
        };

        const result = await this.tapRepo.create(tapData);
        if (!isOk(result)) {
            return result;
        }

        return Ok(mapTapRowToTap(result.value));
    }

    /**
     * Update a tap
     */
    async updateTap(
        tapId: string,
        data: UpdateTapRequest,
        userId: string,
    ): Promise<Result<Tap, NotFoundError | ForbiddenError | Error>> {
        this.logger.info({ tapId, data, userId }, 'Updating tap');

        // Check permission
        const canEdit = await this.canUserEditTap(tapId, userId);
        if (!isOk(canEdit)) {
            return canEdit;
        }

        if (!canEdit.value) {
            return Err(new ForbiddenError('You do not have permission to edit this tap'));
        }

        const result = await this.tapRepo.update(tapId, data);
        if (!isOk(result)) {
            return result;
        }

        return Ok(mapTapRowToTap(result.value));
    }

    /**
     * Delete a tap
     */
    async deleteTap(
        tapId: string,
        userId: string,
    ): Promise<Result<void, NotFoundError | ForbiddenError | Error>> {
        this.logger.info({ tapId, userId }, 'Deleting tap');

        // Only owner can delete
        const tapResult = await this.tapRepo.findById(tapId);
        if (!isOk(tapResult)) {
            return tapResult;
        }

        if (tapResult.value.ownerId !== userId) {
            return Err(new ForbiddenError('Only the owner can delete this tap'));
        }

        return this.tapRepo.delete(tapId);
    }

    /**
     * Get tap members
     */
    async getTapMembers(
        tapId: string,
        requestingUserId?: string,
    ): Promise<Result<TapMember[], NotFoundError | ForbiddenError | Error>> {
        this.logger.debug({ tapId, requestingUserId }, 'Getting tap members');

        // Verify tap exists
        const tapResult = await this.tapRepo.findById(tapId);
        if (!isOk(tapResult)) {
            return tapResult;
        }

        // Check if user has permission to view members of private tap
        if (tapResult.value.isPrivate && requestingUserId) {
            const isMemberResult = await this.tapRepo.isMember(tapId, requestingUserId);
            if (!isOk(isMemberResult)) {
                return isMemberResult;
            }

            if (!isMemberResult.value) {
                return Err(new ForbiddenError('This tap is private'));
            }
        }

        const result = await this.tapRepo.getMembers(tapId);
        if (!isOk(result)) {
            return result;
        }

        return Ok(result.value.map(mapTapMemberRowToTapMember));
    }

    /**
     * Add a member to a tap
     */
    async addMember(
        tapId: string,
        targetUserId: string,
        requestingUserId: string,
        role: string = 'member',
    ): Promise<Result<TapMember, NotFoundError | ForbiddenError | Error>> {
        this.logger.info({ tapId, targetUserId, requestingUserId, role }, 'Adding member to tap');

        // Validate role
        if (!MEMBER_ROLES.includes(role as any)) {
            return Err(new ValidationError(`Invalid role: ${role}`));
        }

        // Check permission
        const canManage = await this.canUserManageMembers(tapId, requestingUserId);
        if (!isOk(canManage)) {
            return canManage;
        }

        if (!canManage.value) {
            return Err(new ForbiddenError('You do not have permission to manage members'));
        }

        // Verify target user exists
        const userResult = await this.userRepo.findById(targetUserId);
        if (!isOk(userResult)) {
            return userResult;
        }

        const result = await this.tapRepo.addMember({
            tapId,
            userId: targetUserId,
            role,
        });

        if (!isOk(result)) {
            return result;
        }

        return Ok(mapTapMemberRowToTapMember(result.value));
    }

    /**
     * Remove a member from a tap
     */
    async removeMember(
        tapId: string,
        targetUserId: string,
        requestingUserId: string,
    ): Promise<Result<void, NotFoundError | ForbiddenError | Error>> {
        this.logger.info({ tapId, targetUserId, requestingUserId }, 'Removing member from tap');

        // Check permission
        const canManage = await this.canUserManageMembers(tapId, requestingUserId);
        if (!isOk(canManage)) {
            return canManage;
        }

        if (!canManage.value) {
            return Err(new ForbiddenError('You do not have permission to manage members'));
        }

        // Cannot remove owner
        const tapResult = await this.tapRepo.findById(tapId);
        if (!isOk(tapResult)) {
            return tapResult;
        }

        if (tapResult.value.ownerId === targetUserId) {
            return Err(new ForbiddenError('Cannot remove the owner from the tap'));
        }

        return this.tapRepo.removeMember(tapId, targetUserId);
    }

    /**
     * Update member role
     */
    async updateMemberRole(
        tapId: string,
        targetUserId: string,
        newRole: string,
        requestingUserId: string,
    ): Promise<Result<TapMember, NotFoundError | ForbiddenError | Error>> {
        this.logger.info({ tapId, targetUserId, newRole, requestingUserId }, 'Updating member role');

        // Validate role
        if (!MEMBER_ROLES.includes(newRole as any)) {
            return Err(new ValidationError(`Invalid role: ${newRole}`));
        }

        // Check permission - must be admin or owner
        const requesterRoleResult = await this.tapRepo.getUserRole(tapId, requestingUserId);
        if (!isOk(requesterRoleResult)) {
            return requesterRoleResult;
        }

        const requesterRole = requesterRoleResult.value;
        if (!requesterRole || (requesterRole !== 'owner' && requesterRole !== 'admin')) {
            return Err(new ForbiddenError('You do not have permission to change member roles'));
        }

        // Cannot change owner's role
        const tapResult = await this.tapRepo.findById(tapId);
        if (!isOk(tapResult)) {
            return tapResult;
        }

        if (tapResult.value.ownerId === targetUserId) {
            return Err(new ForbiddenError('Cannot change the owner\'s role'));
        }

        const result = await this.tapRepo.updateMemberRole(tapId, targetUserId, newRole);
        if (!isOk(result)) {
            return result;
        }

        return Ok(mapTapMemberRowToTapMember(result.value));
    }

    /**
     * Check if user can edit a tap (owner or admin)
     */
    private async canUserEditTap(
        tapId: string,
        userId: string,
    ): Promise<Result<boolean, Error>> {
        const roleResult = await this.tapRepo.getUserRole(tapId, userId);
        if (!isOk(roleResult)) {
            return roleResult;
        }

        const role = roleResult.value;
        return Ok(role === 'owner' || role === 'admin');
    }

    /**
     * Check if user can manage members (owner, admin, or moderator)
     */
    private async canUserManageMembers(
        tapId: string,
        userId: string,
    ): Promise<Result<boolean, Error>> {
        const roleResult = await this.tapRepo.getUserRole(tapId, userId);
        if (!isOk(roleResult)) {
            return roleResult;
        }

        const role = roleResult.value;
        return Ok(role === 'owner' || role === 'admin' || role === 'moderator');
    }
}
