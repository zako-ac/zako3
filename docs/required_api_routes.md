# Required API Routes

This document outlines the required API routes and data schemas for the application. It also tracks whether each API route is currently implemented in the UI codebase.

## 1. Data Structures & Schema

The following data structures define the shapes of resources used throughout the API. They are organized into Core entities, Extended details, and Utility types.

### Core Data Types

Fundamental entities that form the backbone of the application.

#### User

```typescript
interface AuthUser {
  id: string
  discordId: string
  username: string
  avatar: string
  email?: string
  isAdmin: boolean
}

interface User {
  id: string
  discordId: string
  username: string
  avatar: string
  email?: string
  isAdmin: boolean
  isBanned: boolean
  banReason?: string
  banExpiresAt?: string
  createdAt: string
  updatedAt: string
}
```

#### Tap

```typescript
type TapOccupation = 'official' | 'verified' | 'base'
type TapRole = 'music' | 'tts'

type TapPermissionConfig =
  | { type: 'owner_only' }
  | { type: 'public' }
  | { type: 'whitelisted'; userIds: string[] }
  | { type: 'blacklisted'; userIds: string[] }

interface TapBase {
  id: string
  name: string
  description: string
  createdAt: string
  updatedAt: string
  ownerId: string
  occupation: TapOccupation
  roles: TapRole[]
  totalUses: number
}

interface Tap extends TapBase {
  permission: TapPermissionConfig
}

interface TapWithAccess extends Tap {
  hasAccess: boolean
  owner: { id: string; username: string; avatar: string }
}
```

#### Notification

```typescript
type NotificationLevel = 'info' | 'success' | 'warning' | 'error'
type NotificationCategory =
  | 'tap_created'
  | 'tap_updated'
  | 'tap_deleted'
  | 'tap_reported'
  | 'tap_verified'
  | 'tap_verification_requested'
  | 'tap_verification_approved'
  | 'tap_verification_rejected'
  | 'user_banned'
  | 'user_unbanned'
  | 'user_role_changed'
  | 'system_alert'
  | 'custom'

interface Notification {
  id: string
  userId: string
  category: NotificationCategory
  level: NotificationLevel
  title: string
  message: string
  metadata?: Record<string, unknown>
  isRead: boolean
  createdAt: string
}
```

### Extended Data Types

Additional details, statistics, logs, and specific feature extensions.

#### User Extensions

```typescript
interface UserWithActivity extends User {
  lastActiveAt: string
  tapCount: number
  totalTapUses: number
}
```

#### Tap Extensions

```typescript
interface TapStats {
  tapId: string
  currentlyActive: number
  totalUses: number
  cacheHits: number
  uniqueUsers: number
  useRateHistory: { timestamp: string; value: number }[]
  cacheHitRateHistory: { timestamp: string; value: number }[]
}

interface TapAuditLogEntry {
  id: string
  tapId: string
  actorId: string | null
  action: string
  details: string | null
  createdAt: string
}

type TapApiTokenExpiry =
  | '1_month'
  | '3_months'
  | '6_months'
  | '1_year'
  | 'never'

interface TapApiToken {
  id: string
  tapId: string
  label: string
  token: string // Masked
  createdAt: string
  lastUsedAt: string | null
  expiresAt: string | null
}

interface TapApiTokenCreated extends Omit<TapApiToken, 'token'> {
  token: string // Full token displayed once
}
```

#### Admin & Verification

```typescript
interface AdminActivity {
  id: string
  adminId: string
  adminUsername: string
  action: string
  targetType: 'user' | 'tap' | 'notification' | 'system'
  targetId: string
  targetName: string
  timestamp: string
  details?: string
}

type VerificationStatus = 'pending' | 'approved' | 'rejected'

interface VerificationRequestFull {
  id: string
  tapId: string
  tap: TapWithAccess
  reason: string
  evidence?: string
  status: VerificationStatus
  requestedAt: string
  reviewedAt?: string
  reviewedBy?: string
  rejectionReason?: string
}
```

### Misc/Utility Data Types

Request payloads, filter parameters, response wrappers, and common shared types.

#### Common

```typescript
interface PaginationParams {
  page: number
  perPage: number
}

interface PaginationMeta {
  total: number
  page: number
  perPage: number
  totalPages: number
}

interface PaginatedResponse<T> {
  data: T[]
  meta: PaginationMeta
}

type SortDirection = 'asc' | 'desc'

interface ApiError {
  code: string
  message: string
  details?: Record<string, unknown>
}
```

#### Auth Responses

```typescript
interface LoginResponse {
  redirectUrl: string
}

interface AuthCallbackResponse {
  token: string
  user: AuthUser
}

interface RefreshTokenResponse {
  token: string
}
```

#### Inputs & Filters

```typescript
// User Inputs
interface UserFilters {
  search?: string
  isBanned?: boolean
  isAdmin?: boolean
}

interface UserSort {
  field: 'username' | 'createdAt' | 'lastActiveAt' | 'tapCount'
  direction: SortDirection
}

interface BanUserInput {
  reason: string
  expiresAt?: string
}

interface UpdateUserRoleInput {
  isAdmin: boolean
}

// Tap Inputs
interface CreateTapInput {
  id: string // Slug
  name: string
  description: string
  roles: TapRole[]
  permission: TapPermissionConfig
}

interface UpdateTapInput {
  name?: string
  description?: string
  roles?: TapRole[]
  permission?: TapPermissionConfig
}

interface TapFilters {
  search?: string
  roles?: TapRole[]
  accessible?: boolean
  ownerId?: string
}

interface ReportTapInput {
  reason: string
  description: string
}

interface VerificationRequestInput {
  reason: string
  evidence?: string
}

interface CreateTapApiTokenInput {
  label: string
  expiry: TapApiTokenExpiry
}

interface UpdateTapApiTokenInput {
  label: string
}

// Notification Inputs
interface NotificationFilters {
  search?: string
  level?: NotificationLevel
  category?: NotificationCategory
  isRead?: boolean
}
```

## 2. Detailed API

Status Legend:

- ✅ **Implemented in UI**: The frontend has code to call this endpoint.
- ❌ **Not Implemented**: No usage found in the frontend codebase.

### Authentication

| Status | Method | Endpoint         | Description                         | Implementation File        |
| :----- | :----- | :--------------- | :---------------------------------- | :------------------------- |
| ✅     | `GET`  | `/auth/login`    | Get login URL (e.g., Discord OAuth) | `src/features/auth/api.ts` |
| ✅     | `GET`  | `/auth/callback` | Handle OAuth callback               | `src/features/auth/api.ts` |
| ✅     | `POST` | `/auth/logout`   | Logout current user                 | `src/features/auth/api.ts` |
| ✅     | `GET`  | `/auth/refresh`  | Refresh authentication token        | `src/features/auth/api.ts` |

### Users

| Status | Method | Endpoint         | Description                    | Implementation File         |
| :----- | :----- | :--------------- | :----------------------------- | :-------------------------- |
| ✅     | `GET`  | `/users/me`      | Get current user's profile     | `src/features/auth/api.ts`  |
| ✅     | `GET`  | `/users/:userId` | Get public user profile        | `src/features/users/api.ts` |
| ✅     | `GET`  | `/users/me/taps` | Get taps owned by current user | `src/features/taps/api.ts`  |

### Taps

| Status | Method   | Endpoint                 | Description                     | Implementation File        |
| :----- | :------- | :----------------------- | :------------------------------ | :------------------------- |
| ✅     | `GET`    | `/taps`                  | List taps (filtered, paginated) | `src/features/taps/api.ts` |
| ✅     | `POST`   | `/taps`                  | Create a new tap                | `src/features/taps/api.ts` |
| ✅     | `GET`    | `/taps/:tapId`           | Get tap details                 | `src/features/taps/api.ts` |
| ✅     | `PATCH`  | `/taps/:tapId`           | Update a tap                    | `src/features/taps/api.ts` |
| ✅     | `DELETE` | `/taps/:tapId`           | Delete a tap                    | `src/features/taps/api.ts` |
| ✅     | `GET`    | `/taps/:tapId/stats`     | Get tap usage statistics        | `src/features/taps/api.ts` |
| ✅     | `GET`    | `/taps/:tapId/audit-log` | Get tap audit log               | `src/features/taps/api.ts` |
| ✅     | `POST`   | `/taps/:tapId/report`    | Report a tap                    | `src/features/taps/api.ts` |
| ✅     | `POST`   | `/taps/:tapId/verify`    | Request tap verification        | `src/features/taps/api.ts` |

### Tap API Tokens

| Status | Method   | Endpoint                                      | Description               | Implementation File        |
| :----- | :------- | :-------------------------------------------- | :------------------------ | :------------------------- |
| ✅     | `GET`    | `/taps/:tapId/api-tokens`                     | List API tokens for a tap | `src/features/taps/api.ts` |
| ✅     | `POST`   | `/taps/:tapId/api-tokens`                     | Create a new API token    | `src/features/taps/api.ts` |
| ✅     | `PATCH`  | `/taps/:tapId/api-tokens/:tokenId`            | Update API token label    | `src/features/taps/api.ts` |
| ✅     | `POST`   | `/taps/:tapId/api-tokens/:tokenId/regenerate` | Regenerate API token      | `src/features/taps/api.ts` |
| ✅     | `DELETE` | `/taps/:tapId/api-tokens/:tokenId`            | Delete API token          | `src/features/taps/api.ts` |

### Notifications

| Status | Method   | Endpoint                              | Description                    | Implementation File                 |
| :----- | :------- | :------------------------------------ | :----------------------------- | :---------------------------------- |
| ✅     | `GET`    | `/notifications`                      | List user notifications        | `src/features/notifications/api.ts` |
| ✅     | `GET`    | `/notifications/unread-count`         | Get unread notification count  | `src/features/notifications/api.ts` |
| ✅     | `PATCH`  | `/notifications/:notificationId/read` | Mark notification as read      | `src/features/notifications/api.ts` |
| ✅     | `PATCH`  | `/notifications/read-all`             | Mark all notifications as read | `src/features/notifications/api.ts` |
| ✅     | `DELETE` | `/notifications/:notificationId`      | Delete a notification          | `src/features/notifications/api.ts` |

### Admin

| Status | Method  | Endpoint                                  | Description                    | Implementation File                 |
| :----- | :------ | :---------------------------------------- | :----------------------------- | :---------------------------------- |
| ✅     | `GET`   | `/admin/users`                            | List all users (admin)         | `src/features/users/api.ts`         |
| ✅     | `GET`   | `/admin/users/:userId`                    | Get full user details (admin)  | `src/features/users/api.ts`         |
| ✅     | `POST`  | `/admin/users/:userId/ban`                | Ban a user                     | `src/features/users/api.ts`         |
| ✅     | `POST`  | `/admin/users/:userId/unban`              | Unban a user                   | `src/features/users/api.ts`         |
| ✅     | `PATCH` | `/admin/users/:userId/role`               | Update user role               | `src/features/users/api.ts`         |
| ✅     | `GET`   | `/admin/notifications`                    | List notifications (admin)     | `src/features/notifications/api.ts` |
| ✅     | `GET`   | `/admin/activity`                         | Get admin activity logs        | `src/features/admin/api.ts`         |
| ✅     | `GET`   | `/admin/taps/pending-verification`        | List taps pending verification | `src/features/admin/api.ts`         |
| ✅     | `GET`   | `/admin/verifications`                    | List verification requests     | `src/features/admin/api.ts`         |
| ✅     | `POST`  | `/admin/verifications/:requestId/approve` | Approve verification           | `src/features/admin/api.ts`         |
| ✅     | `POST`  | `/admin/verifications/:requestId/reject`  | Reject verification            | `src/features/admin/api.ts`         |
