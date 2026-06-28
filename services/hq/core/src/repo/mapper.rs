use async_trait::async_trait;
use sqlx::{PgPool, Row};
use zako3_tts_matching::{
    Error as TtsError, MapperRepository, Result as TtsResult, WasmMapper, model::MapperInputData,
};

fn db_err(e: sqlx::Error) -> TtsError {
    TtsError::Repository(e.to_string())
}

fn json_err(e: serde_json::Error) -> TtsError {
    TtsError::Repository(e.to_string())
}

fn row_to_mapper(row: &sqlx::postgres::PgRow) -> TtsResult<WasmMapper> {
    let input_data_json: serde_json::Value = row.try_get("input_data").map_err(db_err)?;
    let input_data: Vec<MapperInputData> =
        serde_json::from_value(input_data_json).map_err(json_err)?;

    Ok(WasmMapper {
        id: row.try_get("id").map_err(db_err)?,
        name: row.try_get("name").map_err(db_err)?,
        wasm_bytes: row.try_get("wasm_bytes").map_err(db_err)?,
        sha256_hash: row.try_get("sha256_hash").map_err(db_err)?,
        input_data,
    })
}

pub struct PgMapperRepository {
    pool: PgPool,
}

impl PgMapperRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MapperRepository for PgMapperRepository {
    async fn create(&self, mapper: WasmMapper) -> TtsResult<WasmMapper> {
        let input_data_json = serde_json::to_value(&mapper.input_data).map_err(json_err)?;

        sqlx::query(
            r#"
            INSERT INTO wasm_mappers (id, name, wasm_bytes, sha256_hash, input_data)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(&mapper.id)
        .bind(&mapper.name)
        .bind(&mapper.wasm_bytes)
        .bind(&mapper.sha256_hash)
        .bind(input_data_json)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(mapper)
    }

    async fn find_by_id(&self, id: &str) -> TtsResult<Option<WasmMapper>> {
        let row_opt = sqlx::query(
            r#"
            SELECT id, name, wasm_bytes, sha256_hash, input_data
            FROM wasm_mappers
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;

        match row_opt {
            Some(row) => Ok(Some(row_to_mapper(&row)?)),
            None => Ok(None),
        }
    }

    async fn update(&self, mapper: WasmMapper) -> TtsResult<WasmMapper> {
        let input_data_json = serde_json::to_value(&mapper.input_data).map_err(json_err)?;

        let result = sqlx::query(
            r#"
            UPDATE wasm_mappers
            SET name = $1, wasm_bytes = $2, sha256_hash = $3, input_data = $4, updated_at = NOW()
            WHERE id = $5
            "#,
        )
        .bind(&mapper.name)
        .bind(&mapper.wasm_bytes)
        .bind(&mapper.sha256_hash)
        .bind(input_data_json)
        .bind(&mapper.id)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;

        if result.rows_affected() == 0 {
            return Err(TtsError::NotFound(format!(
                "mapper {} not found",
                mapper.id
            )));
        }

        Ok(mapper)
    }

    async fn delete(&self, id: &str) -> TtsResult<()> {
        sqlx::query("DELETE FROM wasm_mappers WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn list_all(&self) -> TtsResult<Vec<WasmMapper>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, wasm_bytes, sha256_hash, input_data
            FROM wasm_mappers
            ORDER BY id
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;

        let mut mappers = Vec::with_capacity(rows.len());
        for row in &rows {
            mappers.push(row_to_mapper(row)?);
        }
        Ok(mappers)
    }
}
