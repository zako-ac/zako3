import { useQuery, type UseQueryOptions } from '@tanstack/react-query'
import { usePagination } from './use-pagination'
import type { PaginatedResponse, PaginationParams } from '@zako-ac/zako3-data'

/**
 * Options for the usePaginatedQuery hook
 */
interface UsePaginatedQueryOptions<TData, TFilters = Record<string, unknown>> {
  /** Function to generate the query key based on filters and pagination */
  queryKey: (filters: TFilters & Partial<PaginationParams>) => unknown[]
  /** Function to fetch the data */
  queryFn: (
    params: TFilters & Partial<PaginationParams>
  ) => Promise<PaginatedResponse<TData>>
  /** Additional filters to apply to the query */
  filters?: TFilters
  /** Pagination configuration options */
  paginationOptions?: { initialPage?: number; initialPerPage?: number }
  /** Additional React Query options */
  queryOptions?: Omit<
    UseQueryOptions<PaginatedResponse<TData>>,
    'queryKey' | 'queryFn'
  >
}

/**
 * Combined hook for paginated queries that merges pagination state with data fetching.
 * Eliminates the need to manually wire up pagination hooks with query hooks.
 *
 * @template TData - The type of items in the paginated response
 * @template TFilters - The type of filter parameters
 *
 * @example
 * ```typescript
 * const { items, isLoading, paginationInfo, setPage, setPerPage } = usePaginatedQuery({
 *   queryKey: userKeys.list,
 *   queryFn: usersApi.getUsers,
 *   filters: { search: debouncedSearch || undefined },
 * })
 * ```
 */
export function usePaginatedQuery<TData, TFilters = Record<string, unknown>>({
  queryKey,
  queryFn,
  filters = {} as TFilters,
  paginationOptions,
  queryOptions,
}: UsePaginatedQueryOptions<TData, TFilters>) {
  const paginationHook = usePagination(paginationOptions)
  const { pagination, getPaginationInfo } = paginationHook

  const query = useQuery({
    queryKey: queryKey({ ...filters, ...pagination }),
    queryFn: () => queryFn({ ...filters, ...pagination }),
    ...queryOptions,
  })

  return {
    ...query,
    ...paginationHook,
    paginationInfo: getPaginationInfo(query.data?.meta),
    items: query.data?.data ?? [],
  }
}
