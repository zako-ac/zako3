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
 * API tokens table - stores API authentication tokens
 */
export const apiTokens = pgTable(
  'api_tokens',
  {
    id: varchar('id', { length: 100 }).primaryKey(), // UUID
    userId: varchar('user_id', { length: 30 })
      .notNull()
      .references(() => users.id, { onDelete: 'cascade' }),

    // Token details
    name: varchar('name', { length: 255 }).notNull(),
    tokenHash: varchar('token_hash', { length: 255 }).notNull(), // SHA-256 hash of the token
    lastUsedAt: timestamp('last_used_at', { withTimezone: true }),

    // Status
    isRevoked: boolean('is_revoked').notNull().default(false),
    revokedAt: timestamp('revoked_at', { withTimezone: true }),

    // Timestamps
    createdAt: timestamp('created_at', { withTimezone: true })
      .notNull()
      .defaultNow(),
    expiresAt: timestamp('expires_at', { withTimezone: true }),
  },
  (table) => [
    index('idx_api_tokens_user_id').on(table.userId),
    index('idx_api_tokens_token_hash').on(table.tokenHash),
    index('idx_api_tokens_is_revoked').on(table.isRevoked),
  ],
);

export type ApiTokenRow = typeof apiTokens.$inferSelect;
export type NewApiTokenRow = typeof apiTokens.$inferInsert;
