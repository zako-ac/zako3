// Validation Constants
export const TAP_ID_REGEX = /^[a-z0-9_.]+$/;
export const TAP_ID_MIN_LENGTH = 3;
export const TAP_ID_MAX_LENGTH = 32;
export const TAP_NAME_MAX_LENGTH = 64;
export const TAP_DESCRIPTION_MAX_LENGTH = 500;

// Enum-like Constants
export const NOTIFICATION_LEVELS = [
  'info',
  'success',
  'warning',
  'error',
] as const;

export const TAP_ROLES = ['music', 'tts'] as const;

export const TAP_PERMISSION_TYPES = [
  'owner_only',
  'public',
  'whitelisted',
  'blacklisted',
] as const;

// Deprecated: Use TAP_PERMISSION_TYPES instead
export const TAP_PERMISSIONS = TAP_PERMISSION_TYPES;

export const TAP_OCCUPATIONS = ['official', 'verified', 'base'] as const;

export const NOTIFICATION_CATEGORIES = [
  'tap_created',
  'tap_updated',
  'tap_deleted',
  'tap_reported',
  'tap_verified',
  'tap_verification_requested',
  'tap_verification_approved',
  'tap_verification_rejected',
  'user_banned',
  'user_unbanned',
  'user_role_changed',
  'system_alert',
  'custom',
] as const;

export const TAP_API_TOKEN_EXPIRY_OPTIONS = [
  '1_month',
  '3_months',
  '6_months',
  '1_year',
  'never',
] as const;

export const VERIFICATION_STATUSES = [
  'pending',
  'approved',
  'rejected',
] as const;

export const ADMIN_TARGET_TYPES = [
  'user',
  'tap',
  'notification',
  'system',
] as const;

export const SORT_DIRECTIONS = ['asc', 'desc'] as const;

export const PAGE_SIZE_OPTIONS = [10, 20, 50, 100] as const;
export const DEFAULT_PAGE_SIZE = 20;
