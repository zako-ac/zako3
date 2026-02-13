import { z } from 'zod';
import { NOTIFICATION_LEVELS, NOTIFICATION_CATEGORIES } from '../constants';
import { sortDirectionSchema } from './api';

// ============================================================================
// Notification Schemas
// ============================================================================

export const notificationLevelSchema = z.enum(NOTIFICATION_LEVELS);
export const notificationCategorySchema = z.enum(NOTIFICATION_CATEGORIES);

export const notificationSchema = z.object({
  id: z.string(),
  userId: z.string(),
  category: notificationCategorySchema,
  level: notificationLevelSchema,
  title: z.string(),
  message: z.string(),
  metadata: z.record(z.string(), z.unknown()).optional(),
  isRead: z.boolean(),
  createdAt: z.string(),
});

export const notificationFiltersSchema = z.object({
  search: z.string().optional(),
  level: notificationLevelSchema.optional(),
  category: notificationCategorySchema.optional(),
  isRead: z.boolean().optional(),
});

export const notificationSortSchema = z.object({
  field: z.enum(['createdAt', 'level']),
  direction: sortDirectionSchema,
});

// ============================================================================
// Audit Log Schemas
// ============================================================================

export const auditLogEntrySchema = z.object({
  id: z.string(),
  tapId: z.string(),
  actorId: z.string(),
  action: z.string(),
  level: notificationLevelSchema,
  details: z.record(z.string(), z.unknown()),
  createdAt: z.string(),
});

export const auditLogFiltersSchema = z.object({
  search: z.string().optional(),
  level: notificationLevelSchema.optional(),
  action: z.string().optional(),
  actorId: z.string().optional(),
  startDate: z.string().optional(),
  endDate: z.string().optional(),
});

export const tapAuditLogEntrySchema = z.object({
  id: z.string(),
  tapId: z.string(),
  actorId: z.string().nullable(),
  action: z.string(),
  details: z.string().nullable(),
  createdAt: z.string(),
});

// ============================================================================
// Type Exports
// ============================================================================

export type NotificationLevel = z.infer<typeof notificationLevelSchema>;
export type NotificationCategory = z.infer<typeof notificationCategorySchema>;
export type Notification = z.infer<typeof notificationSchema>;
export type NotificationFilters = z.infer<typeof notificationFiltersSchema>;
export type NotificationSort = z.infer<typeof notificationSortSchema>;
export type AuditLogEntry = z.infer<typeof auditLogEntrySchema>;
export type AuditLogFilters = z.infer<typeof auditLogFiltersSchema>;
export type TapAuditLogEntry = z.infer<typeof tapAuditLogEntrySchema>;
