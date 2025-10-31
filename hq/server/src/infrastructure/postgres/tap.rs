use crate::{
    feature::tap::{
        Tap,
        error::TapError,
        repository::{UpdateTap, TapRepository},
    },
    infrastructure::postgres::PostgresDb,
    util::{
        error::{AppError, AppResult, BusinessError},
        snowflake::LazySnowflake,
    },
};

use async_trait::async_trait;
use sqlx::{QueryBuilder, Row, postgres::PgDatabaseError};

fn map_tap_query_error(err: sqlx::Error) -> AppError {
    if let Some(db_err) = err.as_database_error() {
        if let Some(pg_err) = db_err.try_downcast_ref::<PgDatabaseError>() {
            // <table>_<field>_key
            if pg_err.code() == "23505" && pg_err.constraint() == Some("taps_name_key") {
                return AppError::Business(BusinessError::Tap(TapError::DuplicateName));
            }
        }
    }
    err.into()
}

#[async_trait]
impl TapRepository for PostgresDb {
    async fn create_tap(&self, data: Tap) -> AppResult<()> {
        let query = sqlx::query("INSERT INTO taps (id, name) VALUES ($1, $2)")
            .bind(*data.id as i64)
            .bind::<String>(data.name.into());

        query
            .execute(&self.pool)
            .await
            .map_err(map_tap_query_error)?;

        Ok(())
    }

    async fn update_tap(&self, id: LazySnowflake, data: UpdateTap) -> AppResult<()> {
        let mut qb = QueryBuilder::new("UPDATE taps SET ");

        let mut separated = false;

        macro_rules! handle_sep {
            () => {
                if separated {
                    qb.push(", ");
                } else {
                    separated = true;
                }
            };
        }

        if let Some(ref name) = data.name {
            handle_sep!();
            let name_str: String = name.clone().into();
            qb.push("name = ").push_bind(name_str);
        }

        if separated {
            qb.push(" WHERE id = ").push_bind(*id as i64);

            let query = qb.build();
            let result = query
                .execute(&self.pool)
                .await
                .map_err(map_tap_query_error)?;

            if result.rows_affected() > 0 {
                Ok(())
            } else {
                Err(AppError::NotFound)
            }
        } else {
            Ok(())
        }
    }

    async fn delete_tap(&self, id: LazySnowflake) -> AppResult<()> {
        let query = sqlx::query("DELETE FROM taps WHERE id = $1").bind(*id as i64);

        let result = query.execute(&self.pool).await?;

        if result.rows_affected() > 0 {
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }

    async fn find_tap(&self, id: LazySnowflake) -> AppResult<Option<Tap>> {
        let query = sqlx::query("SELECT * FROM taps WHERE id = $1").bind(*id as i64);

        if let Some(item) = query.fetch_optional(&self.pool).await? {
            let id: i64 = item.try_get("id")?;
            let id = id as u64;

            let name: String = item.try_get("name")?;

            let tap = Tap {
                id: id.into(),
                name: name.into(),
            };
            Ok(Some(tap))
        } else {
            Ok(None)
        }
    }
}
