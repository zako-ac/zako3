import { describe, it, expect } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useConfirmDialog } from '../../src/hooks/use-confirm-dialog'

describe('useConfirmDialog', () => {
  it('should initialize with closed state', () => {
    const { result } = renderHook(() => useConfirmDialog())

    expect(result.current.isOpen).toBe(false)
    expect(result.current.selectedItem).toBe(null)
  })

  it('should open dialog with selected item', () => {
    const { result } = renderHook(() =>
      useConfirmDialog<{ id: string; name: string }>()
    )
    const testItem = { id: '1', name: 'Test Item' }

    act(() => {
      result.current.open(testItem)
    })

    expect(result.current.isOpen).toBe(true)
    expect(result.current.selectedItem).toEqual(testItem)
  })

  it('should close dialog and clear selected item', () => {
    const { result } = renderHook(() => useConfirmDialog<{ id: string }>())
    const testItem = { id: '1' }

    act(() => {
      result.current.open(testItem)
    })

    expect(result.current.isOpen).toBe(true)

    act(() => {
      result.current.close()
    })

    expect(result.current.isOpen).toBe(false)
    expect(result.current.selectedItem).toBe(null)
  })

  it('should execute action and close dialog on confirm', async () => {
    const { result } = renderHook(() => useConfirmDialog<{ id: string }>())
    const testItem = { id: '1' }
    let actionExecuted = false

    act(() => {
      result.current.open(testItem)
    })

    await act(async () => {
      await result.current.confirm((item) => {
        expect(item).toEqual(testItem)
        actionExecuted = true
      })
    })

    expect(actionExecuted).toBe(true)
    expect(result.current.isOpen).toBe(false)
    expect(result.current.selectedItem).toBe(null)
  })

  it('should not execute action if no item is selected', async () => {
    const { result } = renderHook(() => useConfirmDialog())
    let actionExecuted = false

    await act(async () => {
      await result.current.confirm(() => {
        actionExecuted = true
      })
    })

    expect(actionExecuted).toBe(false)
  })

  it('should handle async actions', async () => {
    const { result } = renderHook(() => useConfirmDialog<{ id: string }>())
    const testItem = { id: '1' }
    const delay = (ms: number) =>
      new Promise((resolve) => setTimeout(resolve, ms))

    act(() => {
      result.current.open(testItem)
    })

    await act(async () => {
      await result.current.confirm(async (item) => {
        await delay(10)
        expect(item).toEqual(testItem)
      })
    })

    expect(result.current.isOpen).toBe(false)
  })
})
