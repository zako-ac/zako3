/**
 * Pagination metadata type
 */
export interface PaginationMeta {
  page: number;
  perPage: number;
  total: number;
  totalPages: number;
}

/**
 * Pagination parameters for database queries
 */
export interface PaginationParams {
  page: number;
  limit: number;
}

/**
 * Paginated response
 */
export interface PaginatedResponse<T> {
  data: T[];
  meta: PaginationMeta;
}

/**
 * Default pagination limits
 */
export const DEFAULT_PAGE_LIMIT = 20;
export const MAX_PAGE_LIMIT = 100;

/**
 * Validates and normalizes pagination parameters
 */
export function normalizePaginationParams(
  page?: number,
  limit?: number,
): PaginationParams {
  const normalizedPage = Math.max(1, page || 1);
  const normalizedLimit = Math.min(
    Math.max(1, limit || DEFAULT_PAGE_LIMIT),
    MAX_PAGE_LIMIT,
  );

  return {
    page: normalizedPage,
    limit: normalizedLimit,
  };
}

/**
 * Calculates SQL offset from page and limit
 */
export function calculateOffset(page: number, limit: number): number {
  return (page - 1) * limit;
}

/**
 * Creates pagination metadata
 */
export function createPaginationMeta(
  page: number,
  perPage: number,
  total: number,
): PaginationMeta {
  const totalPages = Math.ceil(total / perPage);

  return {
    page,
    perPage,
    total,
    totalPages,
  };
}

/**
 * Helper to create a paginated response
 */
export function createPaginatedResponse<T>(
  data: T[],
  page: number,
  perPage: number,
  total: number,
): PaginatedResponse<T> {
  return {
    data,
    meta: createPaginationMeta(page, perPage, total),
  };
}
