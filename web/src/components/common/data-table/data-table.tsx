import type { ReactNode } from 'react'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { DataTableLoading } from './data-table-loading'
import { DataTableEmpty } from './data-table-empty'

/**
 * Column definition for the DataTable
 */
export interface DataTableColumn<T> {
  /** Unique key for the column */
  key: string
  /** Column header label */
  header: string | ReactNode
  /** Render function for the cell content */
  render: (item: T, index: number) => ReactNode
  /** Optional CSS class for the column */
  className?: string
  /** Optional CSS class for the header */
  headerClassName?: string
}

/**
 * Props for the DataTable component
 */
interface DataTableProps<T> {
  /** Column definitions */
  columns: DataTableColumn<T>[]
  /** Data items to display */
  data: T[]
  /** Loading state */
  isLoading?: boolean
  /** Error state */
  error?: Error | null
  /** Number of skeleton rows to show when loading */
  loadingRowCount?: number
  /** Custom empty state component */
  emptyState?: ReactNode
  /** Function to get unique key for each row */
  getRowKey: (item: T, index: number) => string
  /** Optional CSS class for the table */
  className?: string
  /** Optional callback when a row is clicked */
  onRowClick?: (item: T) => void
}

/**
 * Generic data table component with built-in loading and empty states.
 * Eliminates boilerplate table code across the application.
 *
 * @example
 * ```typescript
 * <DataTable
 *   columns={[
 *     { key: 'name', header: 'Name', render: (user) => user.name },
 *     { key: 'email', header: 'Email', render: (user) => user.email },
 *   ]}
 *   data={users}
 *   isLoading={isLoading}
 *   getRowKey={(user) => user.id}
 * />
 * ```
 */
export function DataTable<T>({
  columns,
  data,
  isLoading = false,
  error = null,
  loadingRowCount = 5,
  emptyState,
  getRowKey,
  className,
  onRowClick,
}: DataTableProps<T>) {
  if (error) {
    return (
      <div className="border-destructive/50 bg-destructive/10 rounded-md border p-4">
        <p className="text-destructive text-sm">
          Error loading data: {error.message}
        </p>
      </div>
    )
  }

  if (isLoading) {
    return <DataTableLoading columns={columns.length} rows={loadingRowCount} />
  }

  if (data.length === 0) {
    return emptyState ? <>{emptyState}</> : <DataTableEmpty />
  }

  return (
    <Table className={className}>
      <TableHeader>
        <TableRow>
          {columns.map((column) => (
            <TableHead key={column.key} className={column.headerClassName}>
              {column.header}
            </TableHead>
          ))}
        </TableRow>
      </TableHeader>
      <TableBody>
        {data.map((item, index) => (
          <TableRow
            key={getRowKey(item, index)}
            onClick={onRowClick ? () => onRowClick(item) : undefined}
            className={onRowClick ? 'cursor-pointer' : undefined}
          >
            {columns.map((column) => (
              <TableCell key={column.key} className={column.className}>
                {column.render(item, index)}
              </TableCell>
            ))}
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}
