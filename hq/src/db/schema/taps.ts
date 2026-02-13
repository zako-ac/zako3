import {
  pgTable,
  varchar,
  text,
  timestamp,
  index,
  integer,
  boolean,
} from 'drizzle-orm/pg-core';
import { users } from './users.js';

/**
 * Taps table - stores tap (community/group) information
 */
export const taps = pgTable(
  'taps',
  {
    id: varchar('id', { length: 50 }).primaryKey(), // Custom tap ID (slug)
    name: varchar('name', { length: 255 }).notNull(),
    description: text('description'),
    
    // Owner
    ownerId: varchar('owner_id', { length: 30 })
      .notNull()
      .references(() => users.id, { onDelete: 'restrict' }),

    // Settings
    isPrivate: boolean('is_private').notNull().default(false),
    isLocked: boolean('is_locked').notNull().default(false),
    
    // Verification
    isVerified: boolean('is_verified').notNull().default(false),
    verifiedAt: timestamp('verified_at', { withTimezone: true }),

    // Stats
    memberCount: integer('member_count').notNull().default(0),

    // Timestamps
    createdAt: timestamp('created_at', { withTimezone: true })
      .notNull()
      .defaultNow(),
    updatedAt: timestamp('updated_at', { withTimezone: true })
      .notNull()
      .defaultNow()
      .$onUpdate(() => new Date()),
  },
  (table) => [
    index('idx_taps_owner_id').on(table.ownerId),
    index('idx_taps_is_verified').on(table.isVerified),
    index('idx_taps_is_private').on(table.isPrivate),
    index('idx_taps_name').on(table.name),
  ],
);

/**
 * Tap members table - stores tap membership and roles
 */
export const tapMembers = pgTable(
  'tap_members',
  {
    tapId: varchar('tap_id', { length: 50 })
      .notNull()
      .references(() => taps.id, { onDelete: 'cascade' }),
    userId: varchar('user_id', { length: 30 })
      .notNull()
      .references(() => users.id, { onDelete: 'cascade' }),
    
    // Role in the tap (owner, admin, moderator, member)
    role: varchar('role', { length: 20 }).notNull().default('member'),

    // Timestamps
    joinedAt: timestamp('joined_at', { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (table) => [
    index('idx_tap_members_tap_id').on(table.tapId),
    index('idx_tap_members_user_id').on(table.userId),
    index('idx_tap_members_role').on(table.role),
  ],
);

/**
 * Tap invitations table - stores pending tap invitations
 */
export const tapInvitations = pgTable(
  'tap_invitations',
  {
    id: varchar('id', { length: 100 }).primaryKey(), // UUID
    tapId: varchar('tap_id', { length: 50 })
      .notNull()
      .references(() => taps.id, { onDelete: 'cascade' }),
    invitedUserId: varchar('invited_user_id', { length: 30 })
      .notNull()
      .references(() => users.id, { onDelete: 'cascade' }),
    invitedByUserId: varchar('invited_by_user_id', { length: 30 })
      .notNull()
      .references(() => users.id, { onDelete: 'cascade' }),

    // Status
    status: varchar('status', { length: 20 }).notNull().default('pending'), // pending, accepted, declined, expired

    // Timestamps
    createdAt: timestamp('created_at', { withTimezone: true })
      .notNull()
      .defaultNow(),
    expiresAt: timestamp('expires_at', { withTimezone: true }),
    respondedAt: timestamp('responded_at', { withTimezone: true }),
  },
  (table) => [
    index('idx_tap_invitations_tap_id').on(table.tapId),
    index('idx_tap_invitations_invited_user_id').on(table.invitedUserId),
    index('idx_tap_invitations_status').on(table.status),
  ],
);

/**
 * Tap audit logs - stores important actions taken on taps
 */
export const tapAuditLogs = pgTable(
  'tap_audit_logs',
  {
    id: varchar('id', { length: 100 }).primaryKey(), // UUID
    tapId: varchar('tap_id', { length: 50 })
      .notNull()
      .references(() => taps.id, { onDelete: 'cascade' }),
    userId: varchar('user_id', { length: 30 })
      .notNull()
      .references(() => users.id, { onDelete: 'set null' }),

    // Action details
    action: varchar('action', { length: 50 }).notNull(), // e.g., 'member_added', 'role_changed', 'settings_updated'
    details: text('details'), // JSON string with additional details

    // Timestamps
    createdAt: timestamp('created_at', { withTimezone: true })
      .notNull()
      .defaultNow(),
  },
  (table) => [
    index('idx_tap_audit_logs_tap_id').on(table.tapId),
    index('idx_tap_audit_logs_user_id').on(table.userId),
    index('idx_tap_audit_logs_action').on(table.action),
    index('idx_tap_audit_logs_created_at').on(table.createdAt),
  ],
);

export type TapRow = typeof taps.$inferSelect;
export type NewTapRow = typeof taps.$inferInsert;
export type TapMemberRow = typeof tapMembers.$inferSelect;
export type NewTapMemberRow = typeof tapMembers.$inferInsert;
export type TapInvitationRow = typeof tapInvitations.$inferSelect;
export type NewTapInvitationRow = typeof tapInvitations.$inferInsert;
export type TapAuditLogRow = typeof tapAuditLogs.$inferSelect;
export type NewTapAuditLogRow = typeof tapAuditLogs.$inferInsert;
