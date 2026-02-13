import {
  pgTable,
  varchar,
  text,
  timestamp,
  index,
} from 'drizzle-orm/pg-core';
import { users } from './users.js';

/**
 * Admin activity table - stores administrative actions
 */
export const adminActivity = pgTable(
  'admin_activity',
  {
    id: varchar('id', { length: 100 }).primaryKey(), // UUID
    adminUserId: varchar('admin_user_id', { length: 30 })
      .notNull()
      .references(() => users.id, { onDelete: 'set null' }),

    // Action details
    action: varchar('action', { length: 50 }).notNull(), // e.g., 'user_banned', 'tap_verified', 'verification_reviewed'
    targetType: varchar('target_type', { length: 50 }), // e.g., 'user', 'tap', 'verification_request'
    targetId: varchar('target_id', { length: 100 }), // ID of the affected resource
    details: text('details'), // JSON string with additional details

    // Timestamps
    createdAt: timestamp('created_at', { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (table) => [
    index('idx_admin_activity_admin_user_id').on(table.adminUserId),
    index('idx_admin_activity_action').on(table.action),
    index('idx_admin_activity_target').on(table.targetType, table.targetId),
    index('idx_admin_activity_created_at').on(table.createdAt),
  ],
);

export type AdminActivityRow = typeof adminActivity.$inferSelect;
export type NewAdminActivityRow = typeof adminActivity.$inferInsert;
