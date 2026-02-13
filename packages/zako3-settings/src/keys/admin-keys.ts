/**
 * @fileoverview Admin settings key definitions.
 *
 * These are the standard admin settings keys as defined in the settings documentation.
 */

import { KeyIdentifier, listType, stringType } from '../types';
import { defineAdminKey, type AdminSettingsKeyDefinition } from './definition';

// =============================================================================
// Admin Settings Keys
// =============================================================================

/**
 * List of admin user IDs.
 */
export const ADMIN_KEY_ADMINS = defineAdminKey({
  identifier: KeyIdentifier('admin.access.admins'),
  friendlyName: 'Admins',
  description: 'List of user IDs who have admin access.',
  valueType: listType(stringType(), { defaultValue: [] }),
});

// =============================================================================
// All Admin Keys Collection
// =============================================================================

/**
 * All defined admin settings keys.
 */
export const ALL_ADMIN_KEYS: readonly AdminSettingsKeyDefinition<unknown>[] = [
  ADMIN_KEY_ADMINS,
] as const;
