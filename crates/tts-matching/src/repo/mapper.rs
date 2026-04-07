use async_trait::async_trait;
use chrono::Utc;

use crate::{
    db::Db,
    model::{MapperInputData, WasmMapper},
    repo::MapperRepository,
    Result,
};

#[derive(Clone)]
pub struct SqliteMapperRepository {
    db: Db,
}

impl SqliteMapperRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MapperRepository for SqliteMapperRepository {
    async fn create(&self, mapper: WasmMapper) -> Result<WasmMapper> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn();
            let now = Utc::now().timestamp();
            let input_data_json = serde_json::to_string(&mapper.input_data)?;

            conn.execute(
                "INSERT INTO wasm_mappers (id, name, wasm_filename, sha256_hash, input_data, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    &mapper.id,
                    &mapper.name,
                    &mapper.wasm_filename,
                    &mapper.sha256_hash,
                    &input_data_json,
                    now,
                    now
                ],
            )?;

            Ok(mapper)
        })
        .await?
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<WasmMapper>> {
        let db = self.db.clone();
        let id = id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn();
            let mut stmt = conn.prepare(
                "SELECT id, name, wasm_filename, sha256_hash, input_data
                 FROM wasm_mappers WHERE id = ?1",
            )?;

            let mut rows = stmt.query(rusqlite::params![&id])?;

            if let Some(row) = rows.next()? {
                Ok(Some(row_to_mapper(row)?))
            } else {
                Ok(None)
            }
        })
        .await?
    }

    async fn update(&self, mapper: WasmMapper) -> Result<WasmMapper> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn();
            let now = Utc::now().timestamp();
            let input_data_json = serde_json::to_string(&mapper.input_data)?;

            let changes = conn.execute(
                "UPDATE wasm_mappers SET name = ?1, wasm_filename = ?2, sha256_hash = ?3, input_data = ?4, updated_at = ?5
                 WHERE id = ?6",
                rusqlite::params![
                    &mapper.name,
                    &mapper.wasm_filename,
                    &mapper.sha256_hash,
                    &input_data_json,
                    now,
                    &mapper.id
                ],
            )?;

            if changes == 0 {
                return Err(crate::Error::NotFound(format!("mapper {} not found", mapper.id)));
            }

            Ok(mapper)
        })
        .await?
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let db = self.db.clone();
        let id = id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn();
            conn.execute("DELETE FROM wasm_mappers WHERE id = ?1", rusqlite::params![&id])?;
            Ok(())
        })
        .await?
    }

    async fn list_all(&self) -> Result<Vec<WasmMapper>> {
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || {
            let conn = db.conn();
            let mut stmt = conn.prepare(
                "SELECT id, name, wasm_filename, sha256_hash, input_data
                 FROM wasm_mappers",
            )?;

            let rows = stmt.query_map([], row_to_mapper)?;

            let mut mappers = Vec::new();
            for row in rows {
                mappers.push(row?);
            }
            Ok(mappers)
        })
        .await?
    }
}

fn row_to_mapper(row: &rusqlite::Row<'_>) -> rusqlite::Result<WasmMapper> {
    let input_data_json: String = row.get(4)?;
    let input_data: Vec<MapperInputData> = serde_json::from_str(&input_data_json)
        .map_err(|e| rusqlite::Error::InvalidParameterName(format!("invalid input_data json: {}", e)))?;

    Ok(WasmMapper {
        id: row.get(0)?,
        name: row.get(1)?,
        wasm_filename: row.get(2)?,
        sha256_hash: row.get(3)?,
        input_data,
    })
}
