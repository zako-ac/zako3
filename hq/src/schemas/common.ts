import { z } from "zod";

/**
 * Common error response schema
 */
export const errorResponseSchema = z.object({
    error: z.object({
        code: z.string().describe("Error code identifier"),
        message: z.string().describe("Human-readable error message"),
        details: z.any().optional().describe("Additional error details"),
    }),
});

export type ErrorResponse = z.infer<typeof errorResponseSchema>;

/**
 * Pagination query parameters
 */
export const paginationQuerySchema = z.object({
    page: z.coerce.number().int().min(1).default(1).describe("Page number"),
    limit: z.coerce
        .number()
        .int()
        .min(1)
        .max(100)
        .default(20)
        .describe("Items per page"),
});

export type PaginationQuery = z.infer<typeof paginationQuerySchema>;

/**
 * Pagination metadata
 */
export const paginationMetaSchema = z.object({
    page: z.number().int().describe("Current page number"),
    perPage: z.number().int().describe("Items per page"),
    total: z.number().int().describe("Total number of items"),
    totalPages: z.number().int().describe("Total number of pages"),
});

export type PaginationMeta = z.infer<typeof paginationMetaSchema>;

/**
 * Paginated response wrapper
 */
export function createPaginatedResponseSchema<T extends z.ZodTypeAny>(
    dataSchema: T
) {
    return z.object({
        data: z.array(dataSchema),
        meta: paginationMetaSchema,
    });
}

/**
 * Generic data wrapper for single item responses
 */
export function createDataResponseSchema<T extends z.ZodTypeAny>(
    dataSchema: T
) {
    return z.object({
        data: dataSchema,
    });
}

/**
 * Success response for operations with no content
 */
export const noContentResponseSchema = z.null().describe("No content");

/**
 * Generic success message
 */
export const successMessageSchema = z.object({
    message: z.string(),
});

/**
 * Common HTTP status code descriptions
 */
export const HTTP_STATUS = {
    OK: 200,
    CREATED: 201,
    NO_CONTENT: 204,
    BAD_REQUEST: 400,
    UNAUTHORIZED: 401,
    FORBIDDEN: 403,
    NOT_FOUND: 404,
    CONFLICT: 409,
    INTERNAL_SERVER_ERROR: 500,
} as const;

/**
 * Common OpenAPI response descriptions
 */
export const RESPONSE_DESCRIPTIONS = {
    OK: "Successful response",
    CREATED: "Resource created successfully",
    NO_CONTENT: "Operation successful with no content",
    BAD_REQUEST: "Invalid request parameters",
    UNAUTHORIZED: "Authentication required",
    FORBIDDEN: "Insufficient permissions",
    NOT_FOUND: "Resource not found",
    CONFLICT: "Resource already exists",
    INTERNAL_SERVER_ERROR: "Internal server error",
} as const;
