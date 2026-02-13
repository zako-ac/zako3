import { z } from 'zod';
import {
  TAP_ID_REGEX,
  TAP_ID_MIN_LENGTH,
  TAP_ID_MAX_LENGTH,
  TAP_NAME_MAX_LENGTH,
  TAP_DESCRIPTION_MAX_LENGTH,
  TAP_ROLES,
  TAP_OCCUPATIONS,
  TAP_API_TOKEN_EXPIRY_OPTIONS,
  VERIFICATION_STATUSES,
} from '../constants';
import { sortDirectionSchema } from './api';
import { userSummarySchema } from './user';

// ============================================================================
// Tap Permission Schemas
// ============================================================================

export const tapPermissionConfigSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('owner_only') }),
  z.object({ type: z.literal('public') }),
  z.object({
    type: z.literal('whitelisted'),
    userIds: z.array(z.string()),
  }),
  z.object({
    type: z.literal('blacklisted'),
    userIds: z.array(z.string()),
  }),
]);

// ============================================================================
// Core Tap Schemas
// ============================================================================

export const tapOccupationSchema = z.enum(TAP_OCCUPATIONS);
export const tapRoleSchema = z.enum(TAP_ROLES);

export const tapBaseSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string(),
  createdAt: z.string(),
  updatedAt: z.string(),
  ownerId: z.string(),
  occupation: tapOccupationSchema,
  roles: z.array(tapRoleSchema),
  totalUses: z.number().int().nonnegative(),
});

export const tapSchema = tapBaseSchema.extend({
  permission: tapPermissionConfigSchema,
});

export const tapWithAccessSchema = tapSchema.extend({
  hasAccess: z.boolean(),
  owner: userSummarySchema,
});

export const timeSeriesPointSchema = z.object({
  timestamp: z.string(),
  value: z.number(),
});

export const tapStatsSchema = z.object({
  tapId: z.string(),
  currentlyActive: z.number().int().nonnegative(),
  totalUses: z.number().int().nonnegative(),
  cacheHits: z.number().int().nonnegative(),
  uniqueUsers: z.number().int().nonnegative(),
  useRateHistory: z.array(timeSeriesPointSchema),
  cacheHitRateHistory: z.array(timeSeriesPointSchema),
});

// ============================================================================
// Tap Filters & Sorting
// ============================================================================

export const tapFiltersSchema = z.object({
  search: z.string().optional(),
  roles: z.array(tapRoleSchema).optional(),
  accessible: z.boolean().optional(),
  ownerId: z.string().optional(),
});

export const tapSortSchema = z.object({
  field: z.enum(['mostUsed', 'recentlyCreated', 'alphabetical']),
  direction: sortDirectionSchema,
});

// ============================================================================
// Tap Input Schemas (with validation)
// ============================================================================

export const tapIdSchema = z
  .string()
  .min(
    TAP_ID_MIN_LENGTH,
    `Tap ID must be at least ${TAP_ID_MIN_LENGTH} characters`
  )
  .max(
    TAP_ID_MAX_LENGTH,
    `Tap ID must be at most ${TAP_ID_MAX_LENGTH} characters`
  )
  .regex(
    TAP_ID_REGEX,
    'Tap ID can only contain lowercase letters, numbers, underscores, and periods'
  );

export const createTapSchema = z.object({
  id: tapIdSchema,
  name: z
    .string()
    .min(1, 'Tap name is required')
    .max(
      TAP_NAME_MAX_LENGTH,
      `Tap name must be at most ${TAP_NAME_MAX_LENGTH} characters`
    ),
  description: z
    .string()
    .max(
      TAP_DESCRIPTION_MAX_LENGTH,
      `Description must be at most ${TAP_DESCRIPTION_MAX_LENGTH} characters`
    )
    .default(''),
  roles: z.array(tapRoleSchema).min(1, 'At least one role is required'),
  permission: tapPermissionConfigSchema.default({ type: 'owner_only' }),
});

export const updateTapSchema = z.object({
  id: tapIdSchema.optional(),
  name: z
    .string()
    .min(1, 'Tap name is required')
    .max(
      TAP_NAME_MAX_LENGTH,
      `Tap name must be at most ${TAP_NAME_MAX_LENGTH} characters`
    )
    .optional(),
  description: z
    .string()
    .max(
      TAP_DESCRIPTION_MAX_LENGTH,
      `Description must be at most ${TAP_DESCRIPTION_MAX_LENGTH} characters`
    )
    .optional(),
  roles: z
    .array(tapRoleSchema)
    .min(1, 'At least one role is required')
    .optional(),
  permission: tapPermissionConfigSchema.optional(),
});

export const tapReportSchema = z.object({
  tapId: z.string(),
  reason: z.enum(['inappropriate', 'spam', 'copyright', 'other']),
  description: z.string().min(10, 'Description must be at least 10 characters'),
});

// Alias for backward compatibility
export const reportTapSchema = z.object({
  reason: z.enum(['inappropriate', 'spam', 'copyright', 'other']),
  description: z.string().min(10, 'Description must be at least 10 characters'),
});

export const tapVerificationRequestSchema = z.object({
  tapId: z.string(),
  reason: z
    .string()
    .min(20, 'Please provide a detailed reason (at least 20 characters)'),
  evidence: z.string().optional(),
});

// Alias for backward compatibility
export const verificationRequestSchema = z.object({
  reason: z
    .string()
    .min(20, 'Please provide a detailed reason (at least 20 characters)'),
  evidence: z.string().optional(),
});

export const verificationStatusSchema = z.enum(VERIFICATION_STATUSES);

export const verificationRequestFullSchema = z.object({
  id: z.string(),
  tapId: z.string(),
  tap: tapWithAccessSchema,
  reason: z.string(),
  evidence: z.string().optional(),
  status: verificationStatusSchema,
  requestedAt: z.string(),
  reviewedAt: z.string().optional(),
  reviewedBy: z.string().optional(),
  rejectionReason: z.string().optional(),
});

// ============================================================================
// Notification Channel Schemas
// ============================================================================

export const notificationChannelSchema = z.object({
  type: z.enum(['email', 'discord_dm', 'webhook']),
  enabled: z.boolean(),
  config: z.record(z.string(), z.string()).optional(),
});

export const tapNotificationSettingsSchema = z.object({
  enabled: z.boolean(),
  channels: z.array(notificationChannelSchema),
});

// ============================================================================
// Tap API Token Schemas
// ============================================================================

export const tapApiTokenExpirySchema = z.enum(TAP_API_TOKEN_EXPIRY_OPTIONS);

export const tapApiTokenSchema = z.object({
  id: z.string(),
  tapId: z.string(),
  label: z.string(),
  token: z.string(), // Masked
  createdAt: z.string(),
  lastUsedAt: z.string().nullable(),
  expiresAt: z.string().nullable(),
});

export const createTapApiTokenSchema = z.object({
  label: z
    .string()
    .min(1, 'Token label is required')
    .max(64, 'Token label must be at most 64 characters'),
  expiry: tapApiTokenExpirySchema,
});

export const updateTapApiTokenSchema = z.object({
  label: z
    .string()
    .min(1, 'Token label is required')
    .max(64, 'Token label must be at most 64 characters'),
});

export const tapApiTokenCreatedSchema = tapApiTokenSchema.omit({ token: true }).extend({
  token: z.string(), // Full token (only returned once on creation)
});

// ============================================================================
// Type Exports
// ============================================================================

export type TapOccupation = z.infer<typeof tapOccupationSchema>;
export type TapRole = z.infer<typeof tapRoleSchema>;
export type TapPermissionConfig = z.infer<typeof tapPermissionConfigSchema>;
export type TapBase = z.infer<typeof tapBaseSchema>;
export type Tap = z.infer<typeof tapSchema>;
export type TapWithAccess = z.infer<typeof tapWithAccessSchema>;
export type TimeSeriesPoint = z.infer<typeof timeSeriesPointSchema>;
export type TapStats = z.infer<typeof tapStatsSchema>;
export type TapFilters = z.infer<typeof tapFiltersSchema>;
export type TapSort = z.infer<typeof tapSortSchema>;
export type CreateTapInput = z.infer<typeof createTapSchema>;
export type UpdateTapInput = z.infer<typeof updateTapSchema>;
export type TapReport = z.infer<typeof tapReportSchema>;
export type ReportTapInput = z.infer<typeof reportTapSchema>; // Alias
export type TapVerificationRequest = z.infer<typeof tapVerificationRequestSchema>;
export type VerificationRequestInput = z.infer<typeof verificationRequestSchema>; // Alias
export type VerificationStatus = z.infer<typeof verificationStatusSchema>;
export type VerificationRequestFull = z.infer<typeof verificationRequestFullSchema>;
export type NotificationChannel = z.infer<typeof notificationChannelSchema>;
export type TapNotificationSettings = z.infer<typeof tapNotificationSettingsSchema>;
export type TapApiTokenExpiry = z.infer<typeof tapApiTokenExpirySchema>;
export type TapApiToken = z.infer<typeof tapApiTokenSchema>;
export type CreateTapApiTokenInput = z.infer<typeof createTapApiTokenSchema>;
export type UpdateTapApiTokenInput = z.infer<typeof updateTapApiTokenSchema>;
export type TapApiTokenCreated = z.infer<typeof tapApiTokenCreatedSchema>;
