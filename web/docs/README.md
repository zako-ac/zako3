# Architecture Documentation

This directory contains architectural documentation for the web application.

## Quick Links

- **[Refactoring Summary](./REFACTORING.md)** - Overview of recent refactoring work
- **[Hooks Guide](./architecture/hooks.md)** - Guide to custom React hooks

## Overview

This codebase follows a feature-based architecture with strong separation of concerns:

```
src/
├── app/              # App configuration (router, providers, query client)
├── components/       # Reusable UI components
│   ├── common/       # Shared components (DataTable, filters, etc.)
│   ├── dashboard/    # Dashboard-specific components
│   ├── layout/       # Layout components (header, sidebar)
│   ├── tap/          # Tap-related components
│   └── ui/           # Base UI primitives (shadcn/ui)
├── features/         # Feature modules (users, taps, auth, etc.)
│   └── [feature]/
│       ├── api.ts    # API methods
│       ├── hooks.ts  # React Query hooks
│       ├── store.ts  # Zustand state (optional)
│       └── index.ts  # Barrel exports
├── hooks/            # Shared custom hooks
├── layouts/          # Page layout wrappers
├── lib/              # Utilities and helpers
├── pages/            # Page components (route handlers)
├── types/            # TypeScript type definitions
└── mocks/            # MSW mock data and handlers
```

## Key Principles

### 1. Feature-Based Organization

Each feature module (`features/[feature]/`) is self-contained with:

- **API methods** - All backend communication
- **React Query hooks** - Data fetching and mutations
- **State management** - Feature-specific state (if needed)
- **Explicit exports** - Clear, tree-shakeable barrel exports

### 2. Component Hierarchy

- **`ui/`** - Primitive, unstyled components (buttons, inputs)
- **`common/`** - Shared business components (DataTable, filters)
- **`[feature]/`** - Feature-specific components
- **`layout/`** - App-wide layout components

### 3. Type Safety

- All types defined in `types/` organized by domain
- Shared types (API, pagination) in `types/api.ts`
- Domain types (User, Tap) in respective files
- Derived types use `Pick`, `Omit`, etc. for consistency

### 4. Data Fetching Pattern

Using React Query with standardized patterns:

```typescript
// Query keys (consistent structure)
export const userKeys = {
  all: ['users'] as const,
  lists: () => [...userKeys.all, 'list'] as const,
  list: (filters) => [...userKeys.lists(), filters] as const,
  details: () => [...userKeys.all, 'detail'] as const,
  detail: (id) => [...userKeys.details(), id] as const,
}

// Query hooks
export const useUsers = (params) => {
  return useQuery({
    queryKey: userKeys.list(params),
    queryFn: () => usersApi.getUsers(params),
  })
}

// Mutation hooks with cache updates
export const useDeleteUser = () => {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: usersApi.deleteUser,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: userKeys.all })
    },
  })
}
```

## Recent Improvements (2026-01-10)

Major refactoring was completed to improve:

- ✅ Code modularity and reusability
- ✅ Type safety and consistency
- ✅ Test coverage
- ✅ Developer experience

See [REFACTORING.md](./REFACTORING.md) for full details.

### New Utilities Available

#### Hooks

- `usePaginatedQuery` - Combined pagination + data fetching
- `useConfirmDialog` - Dialog state management
- `useTableFilters` - Filter + search management

#### Components

- `DataTable` - Generic table with loading/empty states
- `DataTableHeader` - Table header with search and filters

#### Utilities

- `apiCall()` - Simplified API error handling
- Enhanced `LoadingSkeleton` with more variants

See [hooks.md](./architecture/hooks.md) for detailed usage.

## Best Practices

### API Calls

✅ **DO:**

```typescript
import { apiCall } from '@/lib/api-helpers'
return apiCall(apiClient.get<User>('/users/123'))
```

❌ **DON'T:**

```typescript
const response = await apiClient.get<User>('/users/123')
if (response.error) throw new Error(response.error.message)
return response.data
```

### Pagination

✅ **DO:**

```typescript
const { items, paginationInfo, setPage } = usePaginatedQuery({
  queryKey: userKeys.list,
  queryFn: usersApi.getUsers,
  filters: { search },
})
```

❌ **DON'T:**

```typescript
const { pagination } = usePagination()
const { data } = useUsers({
  page: pagination.page,
  perPage: pagination.perPage,
})
const items = data?.data ?? []
```

### Dialog State

✅ **DO:**

```typescript
const deleteDialog = useConfirmDialog<User>()
await deleteDialog.confirm(async (user) => {
  await deleteUser(user.id)
})
```

❌ **DON'T:**

```typescript
const [open, setOpen] = useState(false)
const [selected, setSelected] = useState<User | null>(null)
// ... manual state management
```

## Testing

- **Unit tests** - Hooks, utilities, components
- **Integration tests** - Feature modules (to be added)
- **E2E tests** - Critical user flows (to be added)

Test files live in `tests/__tests__/` and use Vitest + React Testing Library.

## Contributing

When adding new features:

1. **Follow the feature module pattern**
2. **Use existing hooks and components** before creating new ones
3. **Add tests** for new utilities and hooks
4. **Update documentation** for significant changes
5. **Use explicit exports** in barrel files

## Questions?

For specific topics, see:

- [Hooks Guide](./architecture/hooks.md) - Custom hooks usage
- [Refactoring Summary](./REFACTORING.md) - Recent changes
