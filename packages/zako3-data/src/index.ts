// Export all constants
export * from './constants';

// Aliases for clean types (Source of truth: Rust DTOs)
import * as hq_types from './generated/hq';

export * as hq from './generated/hq';

export type Tap = hq_types.TapDto;
export type TapWithAccess = hq_types.TapWithAccessDto;
export type TapStats = hq_types.TapStatsDto;
export type TimeSeriesPoint = hq_types.TimeSeriesPointDto;
export type UserSummary = hq_types.UserSummaryDto;
export type AuthUser = hq_types.AuthUserDto;
export type AuthResponse = hq_types.AuthResponseDto;
export type LoginResponse = hq_types.LoginResponseDto;
export type Notification = hq_types.NotificationDto;
export type AuditLog = hq_types.AuditLogDto;
export type PaginationMeta = hq_types.PaginationMetaDto;
export type PaginatedResponse<T> = hq_types.PaginatedResponseDto<T>;

// API Key / Token Aliases
export type TapApiToken = hq_types.ApiKeyDto;
export type TapApiTokenCreated = hq_types.ApiKeyResponseDto;

// Input Types
export type CreateTapInput = hq_types.CreateTapDto;
export type UpdateTapInput = hq_types.UpdateTapDto;
export type CreateTapApiTokenInput = hq_types.CreateApiKeyDto;
export type UpdateTapApiTokenInput = hq_types.UpdateApiKeyDto;
export type CreateNotificationInput = hq_types.CreateNotificationDto;
export type AuthCallbackInput = hq_types.AuthCallbackDto;

// Enums (Directly from generated)
export type TapOccupation = hq_types.TapOccupation;
export type TapPermission = hq_types.TapPermission;
export type TapRole = hq_types.TapRole;

// Export all schemas and types (after aliases to avoid conflicts if any)
export * from './schemas';

