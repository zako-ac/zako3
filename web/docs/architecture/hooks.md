# Custom Hooks Guide

This guide documents the custom React hooks available in the application and when to use them.

## Table of Contents

- [Pagination Hooks](#pagination-hooks)
- [Dialog & Modal Hooks](#dialog--modal-hooks)
- [Filter & Search Hooks](#filter--search-hooks)
- [Utility Hooks](#utility-hooks)

---

## Pagination Hooks

### `usePagination`

Basic pagination state management.

**Location:** `src/hooks/use-pagination.ts`

**When to use:**

- When you need standalone pagination state
- When you're not using React Query for data fetching

**Example:**

```typescript
import { usePagination } from '@/hooks'

const {
  pagination, // { page, perPage }
  setPage, // (page: number) => void
  setPerPage, // (perPage: number) => void
  nextPage, // () => void
  prevPage, // () => void
  getPaginationInfo, // (meta) => { hasNext, hasPrev, ... }
} = usePagination({
  initialPage: 1,
  initialPerPage: 20,
})
```

---

### `usePaginatedQuery`

**NEW** - Combines pagination with React Query data fetching.

**Location:** `src/hooks/use-paginated-query.ts`

**When to use:**

- When you need paginated data from an API
- When using React Query
- To eliminate boilerplate pagination + query wiring

**Example:**

```typescript
import { usePaginatedQuery } from '@/hooks'
import { userKeys, usersApi } from '@/features/users'

const {
  items, // The data array from response
  isLoading, // Query loading state
  error, // Query error
  paginationInfo, // { hasNext, hasPrev, totalPages, ... }
  pagination, // { page, perPage }
  setPage, // Change page
  setPerPage, // Change page size
  nextPage, // Go to next page
  prevPage, // Go to previous page
} = usePaginatedQuery({
  queryKey: userKeys.list,
  queryFn: usersApi.getUsers,
  filters: { search: debouncedSearch },
  paginationOptions: { initialPage: 1, initialPerPage: 20 },
})
```

**Replaces:**

```typescript
// ❌ Old verbose pattern
const { pagination, setPage, getPaginationInfo } = usePagination()
const { data, isLoading } = useUsers({
  page: pagination.page,
  perPage: pagination.perPage,
  ...filters,
})
const items = data?.data ?? []
const paginationInfo = getPaginationInfo(data?.meta)
```

---

## Dialog & Modal Hooks

### `useConfirmDialog`

**NEW** - Manages confirm dialog state and selected items.

**Location:** `src/hooks/use-confirm-dialog.ts`

**When to use:**

- When you need to show a confirmation dialog
- When you need to track which item is being acted upon
- To eliminate dialog state boilerplate

**Example:**

```typescript
import { useConfirmDialog } from '@/hooks'

const deleteDialog = useConfirmDialog<User>()

// In your component:
<Button onClick={() => deleteDialog.open(user)}>
  Delete
</Button>

<ConfirmDialog
  open={deleteDialog.isOpen}
  onOpenChange={deleteDialog.close}
  title="Delete User"
  description={`Are you sure you want to delete ${deleteDialog.selectedItem?.name}?`}
  onConfirm={() => deleteDialog.confirm(async (user) => {
    await deleteUser(user.id)
    toast.success('User deleted')
  })}
/>
```

**API:**

```typescript
interface UseConfirmDialogReturn<T> {
  isOpen: boolean // Dialog open state
  selectedItem: T | null // Currently selected item
  open: (item: T) => void // Open with item
  close: () => void // Close and clear
  confirm: (action: (item: T) => void | Promise<void>) => Promise<void>
}
```

---

## Filter & Search Hooks

### `useTableFilters`

**NEW** - Manages table filter state with built-in search debouncing.

**Location:** `src/hooks/use-table-filters.ts`

**When to use:**

- When you have a searchable/filterable data table
- When you need debounced search
- To standardize filter state management

**Example:**

```typescript
import { useTableFilters } from '@/hooks'

interface UserFilters {
  search?: string
  role?: string
  isActive?: boolean
}

const {
  search,          // Current search input value
  setSearch,       // Update search input
  debouncedSearch, // Debounced search value
  filters,         // All filter values
  setFilters,      // Set all filters at once
  updateFilter,    // Update single filter
  activeFilters,   // Filters + debounced search for queries
  resetFilters,    // Reset to initial state
} = useTableFilters<UserFilters>({
  initialFilters: { isActive: true },
  searchDebounce: 300, // default: 300ms
})

// Use with queries:
const { data } = useUsers(activeFilters)

// Render:
<SearchInput value={search} onChange={setSearch} />
<FilterDropdown
  value={filters.role}
  onChange={(role) => updateFilter('role', role)}
/>
```

**Combines:**

- ✅ Search state
- ✅ Debounce logic
- ✅ Filter state management
- ✅ Active filter computation

---

### `useDebounce`

Debounces a value (used internally by `useTableFilters`).

**Location:** `src/hooks/use-debounce.ts`

**When to use:**

- When you need to debounce search inputs
- When you want to reduce API calls
- **Prefer** `useTableFilters` for table search

**Example:**

```typescript
import { useDebounce } from '@/hooks'

const [search, setSearch] = useState('')
const debouncedSearch = useDebounce(search, 300)

// Use debouncedSearch in queries
const { data } = useQuery({
  queryKey: ['users', debouncedSearch],
  queryFn: () => fetchUsers(debouncedSearch),
})
```

---

## Utility Hooks

### `useClipboard`

Copy text to clipboard with feedback.

**Location:** `src/hooks/use-clipboard.ts`

**Example:**

```typescript
import { useClipboard } from '@/hooks'

const { copy, copied } = useClipboard()

<Button onClick={() => copy('Text to copy')}>
  {copied ? 'Copied!' : 'Copy'}
</Button>
```

---

### `useIsMobile`

Detect mobile viewport.

**Location:** `src/hooks/use-mobile.ts`

**Example:**

```typescript
import { useIsMobile } from '@/hooks'

const isMobile = useIsMobile()

return isMobile ? <MobileView /> : <DesktopView />
```

---

### `useHotkeys`

Wrapper around `react-hotkeys-hook`.

**Location:** `src/hooks/use-hotkeys.ts`

**Example:**

```typescript
import { useHotkeys } from '@/hooks'

useHotkeys('ctrl+s', () => {
  handleSave()
})
```

---

## Best Practices

### 1. Use the Right Hook for the Job

```typescript
// ❌ Don't manually combine hooks
const { pagination } = usePagination()
const { data } = useQuery(...)
// ... manual wiring

// ✅ Use the combined hook
const { items, paginationInfo } = usePaginatedQuery(...)
```

### 2. Type Your Hooks

```typescript
// ✅ Always provide types
const dialog = useConfirmDialog<User>()
const filters = useTableFilters<UserFilters>()
```

### 3. Consistent Filter Patterns

```typescript
// ✅ Always include search field
interface MyFilters {
  search?: string // Required for useTableFilters
  otherField?: string
}
```

### 4. Combine with Feature Hooks

```typescript
// ✅ Use together
const { activeFilters } = useTableFilters<UserFilters>()
const { items, isLoading, paginationInfo } = usePaginatedQuery({
  queryKey: userKeys.list,
  queryFn: usersApi.getUsers,
  filters: activeFilters, // Pass active filters
})
```

---

## Common Patterns

### Searchable, Paginated Table

```typescript
import { useTableFilters, usePaginatedQuery } from '@/hooks'
import { userKeys, usersApi } from '@/features/users'

function UsersTable() {
  const { search, setSearch, activeFilters } = useTableFilters<UserFilters>()

  const {
    items: users,
    isLoading,
    paginationInfo,
    setPage
  } = usePaginatedQuery({
    queryKey: userKeys.list,
    queryFn: usersApi.getUsers,
    filters: activeFilters,
  })

  return (
    <>
      <SearchInput value={search} onChange={setSearch} />
      <DataTable
        data={users}
        isLoading={isLoading}
        /* ... */
      />
      <DataPagination
        {...paginationInfo}
        onPageChange={setPage}
      />
    </>
  )
}
```

### Delete Confirmation

```typescript
import { useConfirmDialog } from '@/hooks'
import { useDeleteUser } from '@/features/users'

function UserActions({ user }: { user: User }) {
  const deleteDialog = useConfirmDialog<User>()
  const { mutateAsync: deleteUser } = useDeleteUser()

  return (
    <>
      <Button onClick={() => deleteDialog.open(user)}>Delete</Button>

      <ConfirmDialog
        open={deleteDialog.isOpen}
        onOpenChange={deleteDialog.close}
        onConfirm={() => deleteDialog.confirm(async (user) => {
          await deleteUser(user.id)
          toast.success('User deleted')
        })}
      />
    </>
  )
}
```

---

## Testing Hooks

All hooks have comprehensive tests. See:

- `tests/__tests__/use-confirm-dialog.test.ts`
- `tests/__tests__/use-table-filters.test.ts`

Use `@testing-library/react-hooks` for testing:

```typescript
import { renderHook, act } from '@testing-library/react'
import { useConfirmDialog } from '@/hooks'

it('should open dialog with item', () => {
  const { result } = renderHook(() => useConfirmDialog<User>())

  act(() => {
    result.current.open({ id: '1', name: 'Test' })
  })

  expect(result.current.isOpen).toBe(true)
})
```
