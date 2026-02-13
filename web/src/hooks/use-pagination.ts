import { useState, useCallback, useMemo } from 'react'
import type { PaginationParams, PaginationMeta } from '@zako-ac/zako3-data'
import { DEFAULT_PAGE_SIZE } from '@/lib/constants'

interface UsePaginationOptions {
  initialPage?: number
  initialPerPage?: number
}

interface UsePaginationReturn {
  pagination: PaginationParams
  setPage: (page: number) => void
  setPerPage: (perPage: number) => void
  nextPage: () => void
  prevPage: () => void
  goToPage: (page: number) => void
  resetPagination: () => void
  getPaginationInfo: (meta: PaginationMeta | undefined) => {
    hasNext: boolean
    hasPrev: boolean
    totalPages: number
    startItem: number
    endItem: number
    total: number
  }
}

export const usePagination = (
  options: UsePaginationOptions = {}
): UsePaginationReturn => {
  const { initialPage = 1, initialPerPage = DEFAULT_PAGE_SIZE } = options

  const [page, setPage] = useState(initialPage)
  const [perPage, setPerPage] = useState(initialPerPage)

  const pagination = useMemo<PaginationParams>(
    () => ({ page, perPage }),
    [page, perPage]
  )

  const nextPage = useCallback(() => {
    setPage((prev) => prev + 1)
  }, [])

  const prevPage = useCallback(() => {
    setPage((prev) => Math.max(1, prev - 1))
  }, [])

  const goToPage = useCallback((newPage: number) => {
    setPage(Math.max(1, newPage))
  }, [])

  const handlePerPageChange = useCallback((newPerPage: number) => {
    setPerPage(newPerPage)
    setPage(1)
  }, [])

  const resetPagination = useCallback(() => {
    setPage(initialPage)
    setPerPage(initialPerPage)
  }, [initialPage, initialPerPage])

  const getPaginationInfo = useCallback(
    (meta: PaginationMeta | undefined) => {
      if (!meta) {
        return {
          hasNext: false,
          hasPrev: false,
          totalPages: 0,
          startItem: 0,
          endItem: 0,
          total: 0,
        }
      }

      const { total, totalPages } = meta
      const hasNext = page < totalPages
      const hasPrev = page > 1
      const startItem = (page - 1) * perPage + 1
      const endItem = Math.min(page * perPage, total)

      return {
        hasNext,
        hasPrev,
        totalPages,
        startItem,
        endItem,
        total,
      }
    },
    [page, perPage]
  )

  return {
    pagination,
    setPage,
    setPerPage: handlePerPageChange,
    nextPage,
    prevPage,
    goToPage,
    resetPagination,
    getPaginationInfo,
  }
}
