import {
  pgTable,
  varchar,
  boolean,
  timestamp,
  text,
  index,
  unique,
} from 'drizzle-orm/pg-core';

/**
 * Users table - stores Discord user information
 */
export const users = pgTable(
  'users',
  {
    id: varchar('id', { length: 30 }).primaryKey(), // Discord user ID
    username: varchar('username', { length: 255 }).notNull(),
    displayName: varchar('display_name', { length: 255 }),
    avatarUrl: text('avatar_url'),
    discriminator: varchar('discriminator', { length: 4 }),

    // User status
    isBanned: boolean('is_banned').notNull().default(false),
    bannedReason: text('banned_reason'),
    bannedAt: timestamp('banned_at', { withTimezone: true }),

    // Timestamps
    createdAt: timestamp('created_at', { withTimezone: true })
      .notNull()
      .defaultNow(),
    updatedAt: timestamp('updated_at', { withTimezone: true })
      .notNull()
      .defaultNow()
      .$onUpdate(() => new Date()),
    lastSeenAt: timestamp('last_seen_at', { withTimezone: true }),
  },
  (table) => [
    index('idx_users_username').on(table.username),
    index('idx_users_is_banned').on(table.isBanned),
  ],
);

export type UserRow = typeof users.$inferSelect;
export type NewUserRow = typeof users.$inferInsert;
