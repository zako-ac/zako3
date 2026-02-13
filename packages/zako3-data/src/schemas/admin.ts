import { z } from 'zod';
import { ADMIN_TARGET_TYPES } from '../constants';

// ============================================================================
// Admin Schemas
// ============================================================================

export const adminTargetTypeSchema = z.enum(ADMIN_TARGET_TYPES);

export const adminActivitySchema = z.object({
  id: z.string(),
  adminId: z.string(),
  adminUsername: z.string(),
  action: z.string(),
  targetType: adminTargetTypeSchema,
  targetId: z.string(),
  targetName: z.string(),
  timestamp: z.string(),
  details: z.string().optional(),
});

// ============================================================================
// Type Exports
// ============================================================================

export type AdminTargetType = z.infer<typeof adminTargetTypeSchema>;
export type AdminActivity = z.infer<typeof adminActivitySchema>;
