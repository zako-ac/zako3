import { eq, and, or, inArray, isNull, sql, count } from 'drizzle-orm';
import type { PostgresJsDatabase } from 'drizzle-orm/postgres-js';
import type {
  IPersistenceAdapter,
  StoredEntry,
  StoredScope,
  Scope,
  KeyIdentifier,
  SettingsKind,
  EntryQuery,
  ScopeQuery,
  BatchEntryQuery,
  Result,
} from 'zako3-settings';
import { ok, err } from 'zako3-settings';
import type { Logger } from 'pino';
import {
  settingsEntries,
  type SettingsEntryRow,
  type NewSettingsEntryRow,
} from '../../db/schema/index.js';
import type * as schema from '../../db/schema/index.js';

export interface DrizzleAdapterConfig {
  db: PostgresJsDatabase<typeof schema>;
  logger: Logger;
}

function scopeToStoredScope(scope: Scope): StoredScope {
  const stored: StoredScope = {
    kind: scope.kind,
    scope: scope.scope,
  };

  if ('guildId' in scope) {
    (stored as { guildId?: string }).guildId = scope.guildId as string;
  }
  if ('userId' in scope) {
    (stored as { userId?: string }).userId = scope.userId as string;
  }

  return stored;
}

function rowToStoredEntry(row: SettingsEntryRow): StoredEntry {
  return {
    keyIdentifier: row.keyIdentifier,
    value: row.value,
    scope: {
      kind: row.scopeKind as SettingsKind,
      scope: row.scopeType,
      guildId: row.guildId ?? undefined,
      userId: row.userId ?? undefined,
    },
    isImportant: row.isImportant,
  };
}

function storedEntryToRow(entry: StoredEntry): NewSettingsEntryRow {
  return {
    keyIdentifier: entry.keyIdentifier,
    value: entry.value,
    scopeKind: entry.scope.kind,
    scopeType: entry.scope.scope,
    guildId: entry.scope.guildId ?? null,
    userId: entry.scope.userId ?? null,
    isImportant: entry.isImportant,
  };
}

function buildScopeCondition(scope: Scope) {
  const stored = scopeToStoredScope(scope);
  const conditions = [
    eq(settingsEntries.scopeKind, stored.kind),
    eq(settingsEntries.scopeType, stored.scope),
  ];

  if (stored.guildId !== undefined) {
    conditions.push(eq(settingsEntries.guildId, stored.guildId));
  } else {
    conditions.push(isNull(settingsEntries.guildId));
  }

  if (stored.userId !== undefined) {
    conditions.push(eq(settingsEntries.userId, stored.userId));
  } else {
    conditions.push(isNull(settingsEntries.userId));
  }

  return and(...conditions);
}

function buildScopesCondition(scopes: readonly Scope[]) {
  if (scopes.length === 0) {
    return sql`FALSE`;
  }
  if (scopes.length === 1) {
    return buildScopeCondition(scopes[0]);
  }
  return or(...scopes.map(buildScopeCondition));
}

export function createDrizzleAdapter(
  config: DrizzleAdapterConfig
): IPersistenceAdapter {
  const { db, logger } = config;
  const log = logger.child({ module: 'drizzle-adapter' });

  return {
    async getEntry(
      keyIdentifier: KeyIdentifier,
      scope: Scope
    ): Promise<StoredEntry | undefined> {
      const scopeCondition = buildScopeCondition(scope);
      const result = await db
        .select()
        .from(settingsEntries)
        .where(
          and(eq(settingsEntries.keyIdentifier, keyIdentifier as string), scopeCondition)
        )
        .limit(1);

      return result[0] ? rowToStoredEntry(result[0]) : undefined;
    },

    async getEntriesForKey(
      keyIdentifier: KeyIdentifier,
      scopes: readonly Scope[]
    ): Promise<readonly StoredEntry[]> {
      if (scopes.length === 0) {
        return [];
      }

      const scopesCondition = buildScopesCondition(scopes);
      const result = await db
        .select()
        .from(settingsEntries)
        .where(
          and(
            eq(settingsEntries.keyIdentifier, keyIdentifier as string),
            scopesCondition
          )
        );

      return result.map(rowToStoredEntry);
    },

    async getEntriesBatch(
      query: BatchEntryQuery
    ): Promise<ReadonlyMap<string, readonly StoredEntry[]>> {
      const { keyIdentifiers, scopes } = query;

      if (keyIdentifiers.length === 0 || scopes.length === 0) {
        return new Map();
      }

      const keyIds = keyIdentifiers.map((k: KeyIdentifier) => k as string);
      const scopesCondition = buildScopesCondition(scopes);

      const result = await db
        .select()
        .from(settingsEntries)
        .where(
          and(inArray(settingsEntries.keyIdentifier, keyIds), scopesCondition)
        );

      const map = new Map<string, StoredEntry[]>();
      for (const keyId of keyIds) {
        map.set(keyId, []);
      }

      for (const row of result) {
        const entries = map.get(row.keyIdentifier);
        if (entries) {
          entries.push(rowToStoredEntry(row));
        }
      }

      return map;
    },

    async getEntriesByScope(query: ScopeQuery): Promise<readonly StoredEntry[]> {
      const conditions = [eq(settingsEntries.scopeKind, query.settingsKind)];

      if (query.guildId !== undefined) {
        conditions.push(eq(settingsEntries.guildId, query.guildId));
      }
      if (query.userId !== undefined) {
        conditions.push(eq(settingsEntries.userId, query.userId));
      }

      const result = await db
        .select()
        .from(settingsEntries)
        .where(and(...conditions));

      return result.map(rowToStoredEntry);
    },

    async hasEntry(keyIdentifier: KeyIdentifier, scope: Scope): Promise<boolean> {
      const scopeCondition = buildScopeCondition(scope);
      const result = await db
        .select({ id: settingsEntries.id })
        .from(settingsEntries)
        .where(
          and(eq(settingsEntries.keyIdentifier, keyIdentifier as string), scopeCondition)
        )
        .limit(1);

      return result.length > 0;
    },

    async setEntry(entry: StoredEntry): Promise<Result<void>> {
      try {
        const row = storedEntryToRow(entry);
        await db
          .insert(settingsEntries)
          .values(row)
          .onConflictDoUpdate({
            target: [
              settingsEntries.keyIdentifier,
              settingsEntries.scopeKind,
              settingsEntries.scopeType,
              settingsEntries.guildId,
              settingsEntries.userId,
            ],
            set: {
              value: row.value,
              isImportant: row.isImportant,
              updatedAt: new Date(),
            },
          });
        return ok(undefined);
      } catch (error) {
        log.error({ error, entry }, 'Failed to set entry');
        return err(`Database error: ${String(error)}`);
      }
    },

    async setEntriesBatch(entries: readonly StoredEntry[]): Promise<Result<void>> {
      if (entries.length === 0) {
        return ok(undefined);
      }

      try {
        const rows = entries.map(storedEntryToRow);

        await db.transaction(async (tx) => {
          for (const row of rows) {
            await tx
              .insert(settingsEntries)
              .values(row)
              .onConflictDoUpdate({
                target: [
                  settingsEntries.keyIdentifier,
                  settingsEntries.scopeKind,
                  settingsEntries.scopeType,
                  settingsEntries.guildId,
                  settingsEntries.userId,
                ],
                set: {
                  value: row.value,
                  isImportant: row.isImportant,
                  updatedAt: new Date(),
                },
              });
          }
        });

        return ok(undefined);
      } catch (error) {
        log.error({ error }, 'Failed to set entries batch');
        return err(`Database error: ${String(error)}`);
      }
    },

    async deleteEntry(keyIdentifier: KeyIdentifier, scope: Scope): Promise<boolean> {
      const scopeCondition = buildScopeCondition(scope);
      const result = await db
        .delete(settingsEntries)
        .where(
          and(eq(settingsEntries.keyIdentifier, keyIdentifier as string), scopeCondition)
        )
        .returning({ id: settingsEntries.id });

      return result.length > 0;
    },

    async deleteEntriesForKey(keyIdentifier: KeyIdentifier): Promise<number> {
      const result = await db
        .delete(settingsEntries)
        .where(eq(settingsEntries.keyIdentifier, keyIdentifier as string))
        .returning({ id: settingsEntries.id });

      return result.length;
    },

    async deleteEntriesByScope(query: ScopeQuery): Promise<number> {
      const conditions = [eq(settingsEntries.scopeKind, query.settingsKind)];

      if (query.guildId !== undefined) {
        conditions.push(eq(settingsEntries.guildId, query.guildId));
      }
      if (query.userId !== undefined) {
        conditions.push(eq(settingsEntries.userId, query.userId));
      }

      const result = await db
        .delete(settingsEntries)
        .where(and(...conditions))
        .returning({ id: settingsEntries.id });

      return result.length;
    },

    async countEntries(query: EntryQuery): Promise<number> {
      const conditions = [
        eq(settingsEntries.keyIdentifier, query.keyIdentifier as string),
      ];

      if (query.settingsKind) {
        conditions.push(eq(settingsEntries.scopeKind, query.settingsKind));
      }

      if (query.scopes && query.scopes.length > 0) {
        const scopesCondition = buildScopesCondition(query.scopes);
        if (scopesCondition) {
          conditions.push(scopesCondition);
        }
      }

      const result = await db
        .select({ count: count() })
        .from(settingsEntries)
        .where(and(...conditions));

      return result[0]?.count ?? 0;
    },

    async listKeys(settingsKind?: SettingsKind): Promise<readonly string[]> {
      const conditions = settingsKind
        ? [eq(settingsEntries.scopeKind, settingsKind)]
        : [];

      const result = await db
        .selectDistinct({ keyIdentifier: settingsEntries.keyIdentifier })
        .from(settingsEntries)
        .where(conditions.length > 0 ? and(...conditions) : undefined);

      return result.map((r) => r.keyIdentifier);
    },

    async initialize(): Promise<Result<void>> {
      log.info('Drizzle adapter initialized');
      return ok(undefined);
    },

    async close(): Promise<void> {
      log.info('Drizzle adapter closed');
    },

    async isHealthy(): Promise<boolean> {
      try {
        await db.execute(sql`SELECT 1`);
        return true;
      } catch {
        return false;
      }
    },
  };
}
