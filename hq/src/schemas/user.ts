import { z } from "zod";

/**
 * User response schema (public profile)
 */
export const userSchema = z.object({
    id: z.string().describe("Discord user ID"),
    username: z.string().describe("Username"),
    displayName: z.string().optional().describe("Display name"),
    avatarUrl: z.string().optional().describe("Avatar URL"),
    discriminator: z.string().optional().describe("User discriminator"),
    isBanned: z.boolean().describe("Whether user is banned"),
    createdAt: z.string().describe("Account creation timestamp"),
    lastSeenAt: z.string().optional().describe("Last seen timestamp"),
});

export type User = z.infer<typeof userSchema>;

/**
 * User profile schema (authenticated user's own profile)
 * Same as user schema for now, can be extended with private fields later
 */
export const userProfileSchema = userSchema;

export type UserProfile = z.infer<typeof userProfileSchema>;

/**
 * User ID parameter schema
 */
export const userIdParamSchema = z.object({
    userId: z.string().min(1).describe("User ID"),
});

export type UserIdParam = z.infer<typeof userIdParamSchema>;
