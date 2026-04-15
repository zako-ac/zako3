/**
 * Shared API base URL for all MSW handlers.
 * Uses Vite env var if available (for mock mode), otherwise defaults to relative path.
 */
export const API_BASE = import.meta.env.VITE_API_BASE_URL ?? '/api'
