import { describe, it, expect, vi } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useTableFilters } from '../../src/hooks/use-table-filters'

interface TestFilters {
  search?: string
  role?: string
  status?: string
}

describe('useTableFilters', () => {
  it('should initialize with default values', () => {
    const { result } = renderHook(() => useTableFilters())

    expect(result.current.search).toBe('')
    expect(result.current.filters).toEqual({})
    expect(result.current.activeFilters).toEqual({ search: undefined })
  })

  it('should initialize with provided initial filters', () => {
    const { result } = renderHook(() =>
      useTableFilters<TestFilters>({
        initialFilters: { search: 'test', role: 'admin' },
      })
    )

    expect(result.current.search).toBe('test')
    expect(result.current.filters).toEqual({ search: 'test', role: 'admin' })
  })

  it('should update search value', () => {
    const { result } = renderHook(() => useTableFilters())

    act(() => {
      result.current.setSearch('new search')
    })

    expect(result.current.search).toBe('new search')
  })

  it('should debounce search value', async () => {
    vi.useFakeTimers()
    const { result } = renderHook(() =>
      useTableFilters({ searchDebounce: 300 })
    )

    act(() => {
      result.current.setSearch('debounced')
    })

    expect(result.current.debouncedSearch).toBe('')

    act(() => {
      vi.advanceTimersByTime(300)
    })

    expect(result.current.debouncedSearch).toBe('debounced')

    vi.useRealTimers()
  })

  it('should update individual filter', () => {
    const { result } = renderHook(() => useTableFilters<TestFilters>())

    act(() => {
      result.current.updateFilter('role', 'admin')
    })

    expect(result.current.filters).toEqual({ role: 'admin' })

    act(() => {
      result.current.updateFilter('status', 'active')
    })

    expect(result.current.filters).toEqual({ role: 'admin', status: 'active' })
  })

  it('should reset all filters', () => {
    const { result } = renderHook(() =>
      useTableFilters<TestFilters>({
        initialFilters: { search: 'initial', role: 'user' },
      })
    )

    act(() => {
      result.current.setSearch('modified')
      result.current.updateFilter('role', 'admin')
    })

    act(() => {
      result.current.resetFilters()
    })

    expect(result.current.search).toBe('')
    expect(result.current.filters).toEqual({ search: 'initial', role: 'user' })
  })

  it('should include debounced search in activeFilters', async () => {
    vi.useFakeTimers()
    const { result } = renderHook(() =>
      useTableFilters<TestFilters>({ searchDebounce: 300 })
    )

    act(() => {
      result.current.setSearch('active')
      result.current.updateFilter('role', 'admin')
    })

    act(() => {
      vi.advanceTimersByTime(300)
    })

    expect(result.current.activeFilters).toEqual({
      search: 'active',
      role: 'admin',
    })

    vi.useRealTimers()
  })

  it('should exclude empty search from activeFilters', async () => {
    vi.useFakeTimers()
    const { result } = renderHook(() =>
      useTableFilters<TestFilters>({ searchDebounce: 300 })
    )

    act(() => {
      result.current.setSearch('')
      result.current.updateFilter('role', 'admin')
    })

    act(() => {
      vi.advanceTimersByTime(300)
    })

    expect(result.current.activeFilters).toEqual({
      search: undefined,
      role: 'admin',
    })

    vi.useRealTimers()
  })
})
