import { useState, useCallback } from 'react'

/**
 * Return type for the useConfirmDialog hook
 */
interface UseConfirmDialogReturn<T> {
  /** Whether the dialog is currently open */
  isOpen: boolean
  /** The currently selected item (null if none) */
  selectedItem: T | null
  /** Open the dialog with the specified item */
  open: (item: T) => void
  /** Close the dialog and clear the selected item */
  close: () => void
  /** Execute an action with the selected item and close the dialog */
  confirm: (action: (item: T) => void | Promise<void>) => Promise<void>
}

/**
 * Hook for managing confirm dialog state and selected item.
 * Eliminates boilerplate for dialog state management across the application.
 *
 * @template T - The type of item being selected
 *
 * @example
 * ```typescript
 * const deleteDialog = useConfirmDialog<User>()
 *
 * // In your JSX:
 * <Button onClick={() => deleteDialog.open(user)}>Delete</Button>
 *
 * <ConfirmDialog
 *   open={deleteDialog.isOpen}
 *   onOpenChange={deleteDialog.close}
 *   onConfirm={() => deleteDialog.confirm(async (user) => {
 *     await deleteUser(user.id)
 *     toast.success('User deleted')
 *   })}
 * />
 * ```
 */
export function useConfirmDialog<T = unknown>(): UseConfirmDialogReturn<T> {
  const [isOpen, setIsOpen] = useState(false)
  const [selectedItem, setSelectedItem] = useState<T | null>(null)

  const open = useCallback((item: T) => {
    setSelectedItem(item)
    setIsOpen(true)
  }, [])

  const close = useCallback(() => {
    setIsOpen(false)
    setSelectedItem(null)
  }, [])

  const confirm = useCallback(
    async (action: (item: T) => void | Promise<void>) => {
      if (!selectedItem) return
      await action(selectedItem)
      close()
    },
    [selectedItem, close]
  )

  return { isOpen, selectedItem, open, close, confirm }
}
