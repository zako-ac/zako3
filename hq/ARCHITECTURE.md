# HQ Architecture

The HQ service is the backend API and Discord bot for the Zako3 platform. It follows a layered architecture pattern with clear separation of concerns.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     Transport Layer                          │
│  ┌──────────────────────┐    ┌──────────────────────────┐  │
│  │   REST API Routes    │    │    Discord Bot Commands   │  │
│  │   (Fastify)          │    │    (discord.js)           │  │
│  └──────────┬───────────┘    └──────────┬───────────────┘  │
└─────────────┼──────────────────────────┼──────────────────┘
              │                           │
              └─────────┬─────────────────┘
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                    Service Layer                             │
│         (Transport-agnostic business logic)                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │ UserService  │  │  TapService  │  │    ...       │     │
│  └──────┬───────┘  └──────┬───────┘  └──────────────┘     │
└─────────┼──────────────────┼──────────────────────────────┘
          │                  │
          ▼                  ▼
┌─────────────────────────────────────────────────────────────┐
│                  Repository Layer                            │
│          (Data access + Redis caching)                       │
│  ┌───────────────┐  ┌───────────────┐  ┌──────────────┐   │
│  │ UserRepository│  │ TapRepository │  │      ...     │   │
│  └───────┬───────┘  └───────┬───────┘  └──────────────┘   │
└──────────┼──────────────────┼──────────────────────────────┘
           │                  │
           ▼                  ▼
┌─────────────────────────────────────────────────────────────┐
│                   Data Layer                                 │
│  ┌──────────────────┐    ┌──────────────────┐              │
│  │   PostgreSQL     │    │      Redis       │              │
│  │   (Drizzle ORM)  │    │   (ioredis)      │              │
│  └──────────────────┘    └──────────────────┘              │
└─────────────────────────────────────────────────────────────┘
```

## Directory Structure

```
hq/src/
├── api/              # REST API routes (Fastify)
│   ├── context.ts    # Request context types
│   ├── user.routes.ts
│   └── tap.routes.ts
├── services/         # Business logic (transport-agnostic)
│   ├── user.service.ts
│   └── tap.service.ts
├── repositories/     # Data access layer
│   ├── base.repository.ts
│   ├── user.repository.ts
│   └── tap.repository.ts
├── lib/              # Shared utilities
│   ├── result.ts     # Result<T, E> type for error handling
│   ├── errors.ts     # Custom error classes
│   └── pagination.ts # Pagination helpers
├── db/
│   └── schema/       # Drizzle ORM schemas
│       ├── users.ts
│       ├── taps.ts
│       ├── notifications.ts
│       ├── verification.ts
│       ├── api-tokens.ts
│       └── admin.ts
├── bot/              # Discord bot integration
├── auth/             # JWT authentication
├── infra/            # Infrastructure (DB, Redis, logging)
└── config/           # Configuration
```

## Layers Explained

### 1. Transport Layer

**Purpose**: Handle HTTP requests (REST API) or Discord commands (bot)

**Technologies**: Fastify for REST, discord.js for bot

**Key Files**:
- `api/*.routes.ts` - Fastify route handlers
- `bot/*.ts` - Discord bot commands

**Responsibilities**:
- Request validation (Zod schemas)
- Authentication/authorization
- HTTP status codes and error responses
- Calling appropriate service methods

### 2. Service Layer

**Purpose**: Transport-agnostic business logic that can be called from both API and bot

**Key Files**:
- `services/user.service.ts` - User operations
- `services/tap.service.ts` - Tap CRUD, permissions, member management

**Responsibilities**:
- Business rules and validation
- Permission checks
- Orchestrating multiple repository calls
- Data transformation (DB models → API types)

**Design Pattern**: Services return `Result<T, E>` for type-safe error handling

### 3. Repository Layer

**Purpose**: Data access with automatic Redis caching

**Base Class**: `BaseRepository` provides:
- Cache-aside pattern
- Automatic cache invalidation
- TTL support
- Error handling

**Key Files**:
- `repositories/user.repository.ts` - User CRUD
- `repositories/tap.repository.ts` - Tap CRUD, member management

**Responsibilities**:
- SQL queries (via Drizzle ORM)
- Redis caching
- Data persistence

### 4. Data Layer

**Technologies**:
- **PostgreSQL**: Primary data store (via Drizzle ORM)
- **Redis**: Caching layer (via ioredis)

**Database Schemas**:
- `users` - User profiles and ban status
- `taps` - Community/group information
- `tap_members` - Membership and roles
- `tap_invitations` - Pending invitations
- `tap_audit_logs` - Action history
- `notifications` - User notifications
- `verification_requests` - Tap verification
- `api_tokens` - API authentication
- `admin_activity` - Admin action logs

## Key Design Patterns

### Result Type (Rust-inspired)

Instead of throwing exceptions, functions return `Result<T, E>`:

```typescript
type Result<T, E> = 
  | { ok: true; value: T }
  | { ok: false; error: E };

// Usage
const result = await userService.getUser(userId);
if (isOk(result)) {
  console.log(result.value); // User
} else {
  console.error(result.error); // NotFoundError
}
```

### Cache-Aside Pattern

Repositories automatically cache frequently accessed data:

```typescript
async findById(id: string): Promise<Result<User, NotFoundError>> {
  return this.cacheAside(
    `id:${id}`,
    async () => this.db.select().from(users).where(eq(users.id, id)),
    300 // 5 minutes TTL
  );
}
```

### Transport-Agnostic Services

Services don't know about HTTP or Discord - they're pure business logic:

```typescript
// Same service method called from:
// 1. REST API: GET /taps/:tapId
// 2. Discord bot: /tap info <tapId>
await tapService.getTap(tapId, userId);
```

## Error Handling

Custom error classes with HTTP status codes:

```typescript
class NotFoundError extends AppError {
  constructor(message) {
    super(message, 404, 'NOT_FOUND');
  }
}

class ForbiddenError extends AppError {
  constructor(message) {
    super(message, 403, 'FORBIDDEN');
  }
}

// More: ValidationError, UnauthorizedError, ConflictError, etc.
```

## Type Safety

- **Shared types**: `@zako-ac/zako3-data` package used by both web and HQ
- **Runtime validation**: Zod schemas for all inputs
- **Type inference**: TypeScript types inferred from Zod schemas
- **Database types**: Drizzle ORM provides full type safety

## Authentication Flow

1. User authenticates via Discord OAuth
2. HQ issues JWT token
3. Token stored in HTTP-only cookie
4. Middleware validates JWT on each request
5. User context attached to request: `request.requestContext.userId`

## Caching Strategy

- **Cache key pattern**: `{prefix}:{identifier}` (e.g., `user:id:123`)
- **Default TTL**: 5 minutes
- **Invalidation**: Automatic on writes (create, update, delete)
- **Pattern deletion**: Supports wildcards (e.g., `tap:members:*`)

## Next Steps

### To Wire Everything Together:

1. **Update `hq/src/index.ts`**:
   - Instantiate repositories with DB and Redis
   - Instantiate services with repositories
   - Register new API routes
   - Add authentication middleware

2. **Run Database Migrations**:
   ```bash
   cd hq
   npm run db:generate  # Generate migrations
   npm run db:migrate   # Apply migrations
   ```

3. **Implement Remaining Endpoints**:
   - Notifications (GET, PATCH, DELETE)
   - Admin operations (ban/unban, verification)
   - Tap API tokens
   - Audit logs

4. **Discord Bot Integration**:
   - Create `BotServiceWrapper` to provide simplified interface
   - Implement slash commands using services
   - Share business logic with REST API

5. **Add Tests**:
   - Unit tests for services
   - Integration tests for repositories
   - E2E tests for API routes

## Dependencies

- **Fastify**: Web framework
- **Drizzle ORM**: Type-safe SQL queries
- **postgres**: PostgreSQL driver
- **ioredis**: Redis client
- **discord.js**: Discord bot
- **jose**: JWT handling
- **zod**: Runtime validation
- **@zako-ac/zako3-data**: Shared types and schemas
- **pino**: Structured logging

## Performance Considerations

- **Redis caching**: Reduces database load for read-heavy operations
- **Pagination**: All list endpoints support pagination (max 100 items)
- **Database indexes**: Created on frequently queried columns
- **Connection pooling**: PostgreSQL pool (max 20 connections)
- **Lazy evaluation**: Repository methods don't fetch data until needed
