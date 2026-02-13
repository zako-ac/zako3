import { EmptyState } from '@/components/common/empty-state'

/**
 * Empty state for the DataTable component.
 * Displayed when there are no data items to show.
 */
export function DataTableEmpty() {
  return (
    <EmptyState
      title="No data found"
      description="There are no items to display at this time."
    />
  )
}
