//! One-shot importer that migrates legacy SQLite-backed mappers (and their on-disk
//! WASM files) into Postgres.
//!
//! Usage:
//!   MAPPER_WASM_DIR=/var/hq/hq-mappers \
//!   MAPPER_DB_PATH=/var/hq/hq-mappers.db \
//!   DATABASE_URL=postgres://... \
//!   cargo run -p hq-boot --bin import_mappers
//!
//! Idempotent: re-running skips rows already present in Postgres. Pipeline order is
//! overwritten to match the SQLite source.
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use hq_core::run_migrations;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPoolOptions;
use tracing::{info, warn};

struct LegacyMapper {
    id: String,
    name: String,
    wasm_filename: String,
    sha256_hash: String,
    input_data_json: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let wasm_dir = PathBuf::from(
        std::env::var("MAPPER_WASM_DIR").context("MAPPER_WASM_DIR not set")?,
    );
    let sqlite_path = PathBuf::from(
        std::env::var("MAPPER_DB_PATH").context("MAPPER_DB_PATH not set")?,
    );
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL not set")?;

    info!(
        ?wasm_dir,
        ?sqlite_path,
        "starting mapper import from legacy SQLite to Postgres"
    );

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .context("failed to connect to Postgres")?;

    run_migrations(&pool)
        .await
        .context("failed to apply hq-core migrations")?;

    let mappers = read_legacy_mappers(&sqlite_path)?;
    let pipeline = read_legacy_pipeline(&sqlite_path)?;

    info!(
        mappers = mappers.len(),
        pipeline = pipeline.len(),
        "loaded legacy data"
    );

    let mut imported = 0usize;
    let mut skipped = 0usize;
    for m in &mappers {
        let wasm_path = wasm_dir.join(&m.wasm_filename);
        let bytes = match std::fs::read(&wasm_path) {
            Ok(b) => b,
            Err(e) => {
                warn!(mapper_id = %m.id, ?wasm_path, error = %e, "missing wasm file, skipping");
                skipped += 1;
                continue;
            }
        };

        let actual_hash = hex::encode(Sha256::digest(&bytes));
        if actual_hash != m.sha256_hash {
            warn!(
                mapper_id = %m.id,
                expected = %m.sha256_hash,
                actual = %actual_hash,
                "sha256 mismatch between on-disk wasm and sqlite hash, skipping"
            );
            skipped += 1;
            continue;
        }

        let input_data_json: serde_json::Value = serde_json::from_str(&m.input_data_json)
            .with_context(|| format!("invalid input_data JSON for mapper {}", m.id))?;

        let result = sqlx::query(
            r#"
            INSERT INTO wasm_mappers (id, name, wasm_bytes, sha256_hash, input_data)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(&m.id)
        .bind(&m.name)
        .bind(&bytes)
        .bind(&m.sha256_hash)
        .bind(input_data_json)
        .execute(&pool)
        .await
        .with_context(|| format!("failed to insert mapper {}", m.id))?;

        if result.rows_affected() == 0 {
            info!(mapper_id = %m.id, "mapper already present, skipping");
            skipped += 1;
        } else {
            imported += 1;
            info!(mapper_id = %m.id, bytes = bytes.len(), "imported mapper");
        }
    }

    if !pipeline.is_empty() {
        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM pipeline_order")
            .execute(&mut *tx)
            .await?;
        for (position, mapper_id) in pipeline.iter().enumerate() {
            sqlx::query("INSERT INTO pipeline_order (position, mapper_id) VALUES ($1, $2)")
                .bind(position as i32)
                .bind(mapper_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        info!(positions = pipeline.len(), "imported pipeline order");
    }

    info!(imported, skipped, "import complete");
    Ok(())
}

fn read_legacy_mappers(sqlite_path: &PathBuf) -> Result<Vec<LegacyMapper>> {
    let conn = Connection::open(sqlite_path)
        .with_context(|| format!("failed to open sqlite at {}", sqlite_path.display()))?;

    let mut stmt = conn
        .prepare(
            "SELECT id, name, wasm_filename, sha256_hash, input_data FROM wasm_mappers",
        )
        .context("legacy wasm_mappers table missing or invalid")?;

    let rows = stmt
        .query_map([], |row| {
            Ok(LegacyMapper {
                id: row.get(0)?,
                name: row.get(1)?,
                wasm_filename: row.get(2)?,
                sha256_hash: row.get(3)?,
                input_data_json: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| anyhow!("failed to read wasm_mappers: {}", e))?;

    Ok(rows)
}

fn read_legacy_pipeline(sqlite_path: &PathBuf) -> Result<Vec<String>> {
    let conn = Connection::open(sqlite_path)?;
    let mut stmt = conn
        .prepare("SELECT mapper_id FROM pipeline_order ORDER BY position ASC")?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| anyhow!("failed to read pipeline_order: {}", e))?;
    Ok(rows)
}
