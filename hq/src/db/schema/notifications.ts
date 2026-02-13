import {
  pgTable,
  varchar,
  text,
  timestamp,
  index,
  boolean,
} from 'drizzle-orm/pg-core';
import { users } from './users.js';

/**
 * Notifications table - stores user notifications
 */
export const notifications = pgTable(
  'notifications',
  {
    id: varchar('id', { length: 100 }).primaryKey(), // UUID
    userId: varchar('user_id', { length: 30 })
      .notNull()
      .references(() => users.id, { onDelete: 'cascade' }),

    // Notification details
    type: varchar('type', { length: 50 }).notNull(), // e.g., 'tap_invitation', 'member_joined', 'role_changed'
    title: varchar('title', { length: 255 }).notNull(),
    message: text('message'),
    level: varchar('level', { length: 20 }).notNull().default('info'), // info, success, warning, error

    // Links and metadata
    link: text('link'),
    metadata: text('metadata'), // JSON string with additional data

    // Status
    isRead: boolean('is_read').notNull().default(false),
    readAt: timestamp('read_at', { withTimezone: true }),

    // Timestamps
    createdAt: timestamp('created_at', { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (table) => [
    index('idx_notifications_user_id').on(table.userId),
    index('idx_notifications_is_read').on(table.isRead),
    index('idx_notifications_type').on(table.type),
    index('idx_notifications_created_at').on(table.createdAt),
    index('idx_notifications_user_unread').on(table.userId, table.isRead),
  ],
);

export type NotificationRow = typeof notifications.$inferSelect;
export type NewNotificationRow = typeof notifications.$inferInsert;
