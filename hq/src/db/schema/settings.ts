import {
  pgTable,
  serial,
  varchar,
  jsonb,
  boolean,
  timestamp,
  index,
  unique,
} from 'drizzle-orm/pg-core';

export const settingsEntries = pgTable(
  'settings_entries',
  {
    id: serial('id').primaryKey(),

    keyIdentifier: varchar('key_identifier', { length: 255 }).notNull(),

    scopeKind: varchar('scope_kind', { length: 20 }).notNull(),
    scopeType: varchar('scope_type', { length: 30 }).notNull(),
    guildId: varchar('guild_id', { length: 30 }),
    userId: varchar('user_id', { length: 30 }),

    value: jsonb('value').notNull(),
    isImportant: boolean('is_important').notNull().default(false),

    createdAt: timestamp('created_at', { withTimezone: true })
      .notNull()
      .defaultNow(),
    updatedAt: timestamp('updated_at', { withTimezone: true })
      .notNull()
      .defaultNow()
      .$onUpdate(() => new Date()),
  },
  (table) => [
    unique('unique_key_scope').on(
      table.keyIdentifier,
      table.scopeKind,
      table.scopeType,
      table.guildId,
      table.userId
    ),
    index('idx_settings_key').on(table.keyIdentifier),
    index('idx_settings_scope_kind').on(table.scopeKind),
    index('idx_settings_guild').on(table.guildId),
    index('idx_settings_user').on(table.userId),
    index('idx_settings_guild_user').on(table.guildId, table.userId),
  ]
);

export type SettingsEntryRow = typeof settingsEntries.$inferSelect;
export type NewSettingsEntryRow = typeof settingsEntries.$inferInsert;
