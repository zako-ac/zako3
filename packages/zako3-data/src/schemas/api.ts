import { z } from 'zod';
import { SORT_DIRECTIONS } from '../constants';

// ============================================================================
// Common API Schemas
// ============================================================================

export const paginationParamsSchema = z.object({
  page: z.number().int().positive(),
  perPage: z.number().int().positive(),
});

export const paginationMetaSchema = z.object({
  total: z.number().int().nonnegative(),
  page: z.number().int().positive(),
  perPage: z.number().int().positive(),
  totalPages: z.number().int().nonnegative(),
});

export const paginatedResponseSchema = <T extends z.ZodTypeAny>(dataSchema: T) =>
  z.object({
    data: z.array(dataSchema),
    meta: paginationMetaSchema,
  });

export const sortDirectionSchema = z.enum(SORT_DIRECTIONS);

export const apiErrorSchema = z.object({
  code: z.string(),
  message: z.string(),
  details: z.record(z.string(), z.unknown()).optional(),
});

export const apiResponseSchema = <T extends z.ZodTypeAny>(dataSchema: T) =>
  z.object({
    data: dataSchema,
    error: apiErrorSchema.optional(),
  });

// ============================================================================
// Type Exports
// ============================================================================

export type PaginationParams = z.infer<typeof paginationParamsSchema>;
export type PaginationMeta = z.infer<typeof paginationMetaSchema>;
export type PaginatedResponse<T> = {
  data: T[];
  meta: PaginationMeta;
};
export type SortDirection = z.infer<typeof sortDirectionSchema>;
export type ApiError = z.infer<typeof apiErrorSchema>;
export type ApiResponse<T> = {
  data: T;
  error?: ApiError;
};
