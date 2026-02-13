import { z } from "zod";
import { paginationQuerySchema } from "./common.js";

/**
 * Tap member role enum
 */
export const tapMemberRoleSchema = z.enum([
    "owner",
    "admin",
    "moderator",
    "member",
]);

export type TapMemberRole = z.infer<typeof tapMemberRoleSchema>;

/**
 * Tap response schema
 */
export const tapSchema = z.object({
    id: z.string().describe("Tap ID (slug)"),
    name: z.string().describe("Tap name"),
    description: z.string().optional().describe("Tap description"),
    ownerId: z.string().describe("Owner user ID"),
    isPrivate: z.boolean().describe("Whether tap is private"),
    isLocked: z.boolean().describe("Whether tap is locked"),
    isVerified: z.boolean().describe("Whether tap is verified"),
    memberCount: z.number().int().describe("Number of members"),
    createdAt: z.string().describe("Creation timestamp"),
    updatedAt: z.string().describe("Last update timestamp"),
});

export type Tap = z.infer<typeof tapSchema>;

/**
 * Tap member schema
 */
export const tapMemberSchema = z.object({
    tapId: z.string().describe("Tap ID"),
    userId: z.string().describe("User ID"),
    role: z.string().describe("Member role"),
    joinedAt: z.string().describe("Join timestamp"),
});

export type TapMember = z.infer<typeof tapMemberSchema>;

/**
 * Create tap request schema
 */
export const createTapRequestSchema = z.object({
    id: z
        .string()
        .min(3)
        .max(50)
        .regex(/^[a-z0-9-]+$/)
        .describe("Tap ID (slug, lowercase alphanumeric with hyphens)"),
    name: z.string().min(1).max(255).describe("Tap name"),
    description: z.string().max(1000).optional().describe("Tap description"),
    isPrivate: z.boolean().optional().describe("Whether tap is private"),
});

export type CreateTapRequest = z.infer<typeof createTapRequestSchema>;

/**
 * Update tap request schema
 */
export const updateTapRequestSchema = z.object({
    name: z.string().min(1).max(255).optional().describe("Tap name"),
    description: z.string().max(1000).optional().describe("Tap description"),
    isPrivate: z.boolean().optional().describe("Whether tap is private"),
    isLocked: z.boolean().optional().describe("Whether tap is locked"),
});

export type UpdateTapRequest = z.infer<typeof updateTapRequestSchema>;

/**
 * List taps query parameters
 */
export const listTapsQuerySchema = paginationQuerySchema.extend({
    search: z.string().optional().describe("Search query for tap name"),
    isVerified: z.coerce
        .boolean()
        .optional()
        .describe("Filter by verified status"),
    ownerId: z.string().optional().describe("Filter by owner ID"),
});

export type ListTapsQuery = z.infer<typeof listTapsQuerySchema>;

/**
 * Tap ID parameter schema
 */
export const tapIdParamSchema = z.object({
    tapId: z.string().min(1).describe("Tap ID"),
});

export type TapIdParam = z.infer<typeof tapIdParamSchema>;
