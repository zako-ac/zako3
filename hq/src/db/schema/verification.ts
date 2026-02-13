import {
  pgTable,
  varchar,
  text,
  timestamp,
  index,
} from 'drizzle-orm/pg-core';
import { users } from './users.js';
import { taps } from './taps.js';

/**
 * Verification requests table - stores tap verification requests
 */
export const verificationRequests = pgTable(
  'verification_requests',
  {
    id: varchar('id', { length: 100 }).primaryKey(), // UUID
    tapId: varchar('tap_id', { length: 50 })
      .notNull()
      .references(() => taps.id, { onDelete: 'cascade' }),
    requestedByUserId: varchar('requested_by_user_id', { length: 30 })
      .notNull()
      .references(() => users.id, { onDelete: 'cascade' }),

    // Request details
    reason: text('reason').notNull(),
    status: varchar('status', { length: 20 }).notNull().default('pending'), // pending, approved, rejected

    // Review details
    reviewedByUserId: varchar('reviewed_by_user_id', { length: 30 })
      .references(() => users.id, { onDelete: 'set null' }),
    reviewNotes: text('review_notes'),

    // Timestamps
    createdAt: timestamp('created_at', { withTimezone: true })
      .notNull()
      .defaultNow(),
    reviewedAt: timestamp('reviewed_at', { withTimezone: true }),
  },
  (table) => [
    index('idx_verification_requests_tap_id').on(table.tapId),
    index('idx_verification_requests_status').on(table.status),
    index('idx_verification_requests_created_at').on(table.createdAt),
  ],
);

export type VerificationRequestRow = typeof verificationRequests.$inferSelect;
export type NewVerificationRequestRow = typeof verificationRequests.$inferInsert;
