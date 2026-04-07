// Re-export data constants from shared package
export * from '@zako-ac/zako3-data';

// Web-specific constants
export const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || '/api';
export const WS_BASE_URL =
  import.meta.env.VITE_WS_BASE_URL || 'ws://localhost:8080';

export const AUTH_TOKEN_KEY = 'zako_auth_token';
export const AUTH_USER_KEY = 'zako_auth_user';

export const THEME_STORAGE_KEY = 'zako-ui-theme';

export const ROUTES = {
  HOME: '/',
  LOGIN: '/login',
  AUTH_CALLBACK: '/auth/callback',
  DASHBOARD: '/dashboard',
  SETTINGS: '/settings',
  TAPS: '/taps',
  TAPS_CREATE: '/taps/create',
  TAPS_MINE: '/taps/mine',
  TAP_SETTINGS: (tapId: string) => `/taps/${tapId}/settings`,
  TAP_STATS: (tapId: string) => `/taps/${tapId}/stats`,
  TAP_API_KEYS: (tapId: string) => `/taps/${tapId}/api-keys`,
  TAP_VERIFICATION: (tapId: string) => `/taps/${tapId}/verification`,
  ADMIN: '/admin',
  ADMIN_USERS: '/admin/users',
  ADMIN_USER: (userId: string) => `/admin/users/${userId}`,
  ADMIN_TAPS: '/admin/taps',
  ADMIN_TAP: (tapId: string) => `/admin/taps/${tapId}`,
  ADMIN_NOTIFICATIONS: '/admin/notifications',
  ADMIN_VERIFICATIONS: '/admin/verifications',
  ADMIN_SETTINGS: '/admin/settings',
  ADMIN_MAPPERS: '/admin/mappers',
  ADMIN_MAPPERS_PIPELINE: '/admin/mappers/pipeline',
  VOICE_CHANNEL: (guildId: string, channelId: string) => `/voice/${guildId}/${channelId}`,
  GUILD_SETTINGS: (guildId: string) => `/guilds/${guildId}/settings`,
  GUILD_SETTINGS_ME: (guildId: string) => `/guilds/${guildId}/settings/me`,
  GUILD_SETTINGS_GUILD: (guildId: string) => `/guilds/${guildId}/settings/guild`,
} as const;
