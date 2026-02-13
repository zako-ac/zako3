import { drizzle } from 'drizzle-orm/postgres-js';
import { migrate } from 'drizzle-orm/postgres-js/migrator';
import postgres from 'postgres';

async function main() {
  const databaseUrl = process.env.DATABASE_URL;

  if (!databaseUrl) {
    console.error('[migrate] DATABASE_URL environment variable is required');
    process.exit(1);
  }

  console.log('[migrate] Starting database migration...');

  const sql = postgres(databaseUrl, { max: 1 });
  const db = drizzle(sql);

  try {
    await migrate(db, { migrationsFolder: './src/db/migrations' });
    console.log('[migrate] Migrations completed successfully');
    await sql.end();
    process.exit(0);
  } catch (error) {
    console.error('[migrate] Migration failed:', error);
    await sql.end();
    process.exit(1);
  }
}

main();
