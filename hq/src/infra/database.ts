import postgres from 'postgres';
import { drizzle } from 'drizzle-orm/postgres-js';
import type { Logger } from 'pino';
import * as schema from '../db/schema/index.js';

export interface DatabaseConfig {
  url: string;
  max?: number;
  idleTimeout?: number;
  connectTimeout?: number;
}

export interface Database {
  db: ReturnType<typeof drizzle<typeof schema>>;
  sql: postgres.Sql;
  close: () => Promise<void>;
  isHealthy: () => Promise<boolean>;
}

export function createDatabase(config: DatabaseConfig, logger: Logger): Database {
  const log = logger.child({ module: 'database' });

  const sql = postgres(config.url, {
    max: config.max ?? 20,
    idle_timeout: config.idleTimeout ?? 30,
    connect_timeout: config.connectTimeout ?? 10,
    onnotice: (notice) => log.debug({ notice }, 'pg notice'),
  });

  const db = drizzle(sql, { schema });

  return {
    db,
    sql,
    async close() {
      log.info('Closing database connection pool');
      await sql.end();
    },
    async isHealthy() {
      try {
        await sql`SELECT 1`;
        return true;
      } catch (error) {
        log.warn({ error }, 'Database health check failed');
        return false;
      }
    },
  };
}
