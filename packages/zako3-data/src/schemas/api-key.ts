import { z } from 'zod';
import { TAP_API_TOKEN_EXPIRY_OPTIONS } from '../constants';

// ============================================================================
// User API Key (Personal Access Token) Schemas
//
// User-scoped JWTs that carry the owner's permissions. The full token is shown
// only once at creation; it is revocable.
// ============================================================================

export const userApiKeyExpirySchema = z.enum(TAP_API_TOKEN_EXPIRY_OPTIONS);

export const userApiKeySchema = z.object({
    id: z.string(),
    label: z.string(),
    createdAt: z.string(),
    lastUsedAt: z.string().nullable(),
    expiresAt: z.string().nullable(),
    revoked: z.boolean(),
});

export const createUserApiKeySchema = z.object({
    label: z
        .string()
        .min(1, 'Key label is required')
        .max(64, 'Key label must be at most 64 characters'),
    expiry: userApiKeyExpirySchema,
});

export const updateUserApiKeySchema = z.object({
    label: z
        .string()
        .min(1, 'Key label is required')
        .max(64, 'Key label must be at most 64 characters'),
});

export const userApiKeyCreatedSchema = userApiKeySchema.extend({
    token: z.string(), // Full JWT (only returned once on creation)
});

// ============================================================================
// Type Exports
// ============================================================================

export type UserApiKeyExpiry = z.infer<typeof userApiKeyExpirySchema>;
export type UserApiKey = z.infer<typeof userApiKeySchema>;
export type CreateUserApiKeyInput = z.infer<typeof createUserApiKeySchema>;
export type UpdateUserApiKeyInput = z.infer<typeof updateUserApiKeySchema>;
export type UserApiKeyCreated = z.infer<typeof userApiKeyCreatedSchema>;
