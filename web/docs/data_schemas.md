# Data Schemas

This document outlines the TypeScript interfaces and types used within the web application, reflecting the data structures for API responses and internal logic.

## Common

```typescript
export interface PaginationParams {
  page: number
  perPage: number
}

export interface PaginationMeta {
  total: number
  page: number
  perPage: number
  totalPages: number
}

export interface PaginatedResponse<T> {
  data: T[]
  meta: PaginationMeta
}

export type SortDirection = 'asc' | 'desc'

export interface ApiError {
  code: string
  message: string
  details?: Record<string, unknown>
}

export interface ApiResponse<T> {
  data: T
  error?: ApiError
}
```

## Auth

```typescript
export interface AuthState {
  isAuthenticated: boolean
  user: AuthUser | null
  token: string | null
}

export interface AuthUser {
  id: string
  discordId: string
  username: string
  avatar: string
  email?: string
  isAdmin: boolean
}

export interface LoginResponse {
  redirectUrl: string
}

export interface AuthCallbackResponse {
  token: string
  user: AuthUser
}

export interface RefreshTokenResponse {
  token: string
}
```

## User

```typescript
export interface User {
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

export interface UserWithActivity extends User {
  lastActiveAt: string
  tapCount: number
  totalTapUses: number
}

export interface UserFilters {
  search?: string
  isBanned?: boolean
  isAdmin?: boolean
}

export interface UserSort {
  field: 'username' | 'createdAt' | 'lastActiveAt' | 'tapCount'
  direction: SortDirection
}

export interface BanUserInput {
  userId: string
  reason: string
  expiresAt?: string
}

export interface UpdateUserRoleInput {
  userId: string
  isAdmin: boolean
}
```

## Tap

```typescript
export type TapOccupation = 'official' | 'verified' | 'base'
export type TapRole = 'music' | 'tts'

export type TapPermissionConfig =
  | { type: 'owner_only' }
  | { type: 'public' }
  | { type: 'whitelisted'; userIds: string[] }
  | { type: 'blacklisted'; userIds: string[] }

export interface TapBase {
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

export interface Tap extends TapBase {
  permission: TapPermissionConfig
}

export type UserSummary = Pick<User, 'id' | 'username' | 'avatar'>

export interface TapWithAccess extends Tap {
  hasAccess: boolean
  owner: UserSummary
}

export interface TapStats {
  tapId: string
  currentlyActive: number
  totalUses: number
  cacheHits: number
  uniqueUsers: number
  useRateHistory: TimeSeriesPoint[]
  cacheHitRateHistory: TimeSeriesPoint[]
}

export interface TimeSeriesPoint {
  timestamp: string
  value: number
}

export interface TapFilters {
  search?: string
  roles?: TapRole[]
  accessible?: boolean
  ownerId?: string
}

export interface TapSort {
  field: 'mostUsed' | 'recentlyCreated' | 'alphabetical'
  direction: SortDirection
}

export interface CreateTapInput {
  id: string
  name: string
  description: string
  roles: TapRole[]
  permission: TapPermissionConfig
}

export interface UpdateTapInput {
  id?: string
  name?: string
  description?: string
  roles?: TapRole[]
  permission?: TapPermissionConfig
}

export interface TapReport {
  tapId: string
  reason: string
  description: string
}

export interface TapVerificationRequest {
  tapId: string
  reason: string
  evidence?: string
}

export type VerificationStatus = 'pending' | 'approved' | 'rejected'

export interface VerificationRequestFull {
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

export type TapApiTokenExpiry =
  | '1_month'
  | '3_months'
  | '6_months'
  | '1_year'
  | 'never'

export interface TapApiToken {
  id: string
  tapId: string
  label: string
  token: string // Masked
  createdAt: string
  lastUsedAt: string | null
  expiresAt: string | null
}

export interface CreateTapApiTokenInput {
  label: string
  expiry: TapApiTokenExpiry
}

export interface UpdateTapApiTokenInput {
  label: string
}

export interface TapApiTokenCreated extends Omit<TapApiToken, 'token'> {
  token: string // Full token
}
```

## Notification

```typescript
export type NotificationLevel = 'info' | 'success' | 'warning' | 'error'

export type NotificationCategory =
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

export interface Notification {
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

export interface NotificationFilters {
  search?: string
  level?: NotificationLevel
  category?: NotificationCategory
  isRead?: boolean
}

export interface NotificationSort {
  field: 'createdAt' | 'level'
  direction: SortDirection
}

export interface AuditLogEntry {
  id: string
  tapId: string
  actorId: string
  action: string
  level: NotificationLevel
  details: Record<string, unknown>
  createdAt: string
}

export interface AuditLogFilters {
  search?: string
  level?: NotificationLevel
  action?: string
  actorId?: string
  startDate?: string
  endDate?: string
}

export interface TapAuditLogEntry {
  id: string
  tapId: string
  actorId: string | null
  action: string
  details: string | null
  createdAt: string
}
```

## Admin

```typescript
export interface AdminActivity {
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
```
