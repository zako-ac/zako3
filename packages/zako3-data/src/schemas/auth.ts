import { z } from 'zod';

// ============================================================================
// Auth Schemas
// ============================================================================

export const authUserSchema = z.object({
  id: z.string(),
  discordId: z.string(),
  username: z.string(),
  avatar: z.string(),
  email: z.string().optional(),
  isAdmin: z.boolean(),
});

export const authStateSchema = z.object({
  isAuthenticated: z.boolean(),
  user: authUserSchema.nullable(),
  token: z.string().nullable(),
});

export const loginResponseSchema = z.object({
  redirectUrl: z.string().url(),
});

export const authCallbackResponseSchema = z.object({
  token: z.string(),
  user: authUserSchema,
});

export const refreshTokenResponseSchema = z.object({
  token: z.string(),
});

// ============================================================================
// Type Exports
// ============================================================================

export type AuthUser = z.infer<typeof authUserSchema>;
export type AuthState = z.infer<typeof authStateSchema>;
export type LoginResponse = z.infer<typeof loginResponseSchema>;
export type AuthCallbackResponse = z.infer<typeof authCallbackResponseSchema>;
export type RefreshTokenResponse = z.infer<typeof refreshTokenResponseSchema>;
