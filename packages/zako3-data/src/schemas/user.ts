import { z } from 'zod';
import { sortDirectionSchema } from './api';

// ============================================================================
// User Schemas
// ============================================================================

export const userSchema = z.object({
  id: z.string(),
  discordId: z.string(),
  username: z.string(),
  avatar: z.string(),
  email: z.string().optional(),
  isAdmin: z.boolean(),
  isBanned: z.boolean(),
  banReason: z.string().optional(),
  banExpiresAt: z.string().optional(),
  createdAt: z.string(),
  updatedAt: z.string(),
});

export const userWithActivitySchema = userSchema.extend({
  lastActiveAt: z.string(),
  tapCount: z.number().int().nonnegative(),
  totalTapUses: z.number().int().nonnegative(),
});

export const userSummarySchema = userSchema.pick({
  id: true,
  username: true,
  avatar: true,
});

export const userFiltersSchema = z.object({
  search: z.string().optional(),
  isBanned: z.boolean().optional(),
  isAdmin: z.boolean().optional(),
});

export const userSortSchema = z.object({
  field: z.enum(['username', 'createdAt', 'lastActiveAt', 'tapCount']),
  direction: sortDirectionSchema,
});

export const banUserInputSchema = z.object({
  userId: z.string(),
  reason: z.string().min(1, 'Ban reason is required'),
  expiresAt: z.string().optional(),
});

export const updateUserRoleInputSchema = z.object({
  userId: z.string(),
  isAdmin: z.boolean(),
});

// ============================================================================
// Type Exports
// ============================================================================

export type User = z.infer<typeof userSchema>;
export type UserWithActivity = z.infer<typeof userWithActivitySchema>;
export type UserSummary = z.infer<typeof userSummarySchema>;
export type UserFilters = z.infer<typeof userFiltersSchema>;
export type UserSort = z.infer<typeof userSortSchema>;
export type BanUserInput = z.infer<typeof banUserInputSchema>;
export type UpdateUserRoleInput = z.infer<typeof updateUserRoleInputSchema>;
