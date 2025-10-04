use crate::{
    controller::identity::{
        entity::Identity,
        error::{IdentityError, IdentityResult},
        repository::{IdentityRepository, UpdateIdentity},
    },
    infra::postgres::PostgresDb,
    util::snowflake::LazySnowflake,
};

use sqlx::Row;

impl IdentityRepository for PostgresDb {
    async fn create_identity(&self, data: &Identity) -> IdentityResult<()> {
        let query = sqlx::query("INSERT INTO identity (id, name) VALUES ($1, $2)")
            .bind(*data.id as i64)
            .bind(&data.name);

        query.execute(&self.pool).await?;

        Ok(())
    }

    async fn update_identity(
        &self,
        id: LazySnowflake,
        data: &UpdateIdentity,
    ) -> Result<(), IdentityError> {
        let query = sqlx::query("UPDATE identity SET name = $1 WHERE id = $2")
            .bind(&data.name)
            .bind(*id as i64);

        query.execute(&self.pool).await?;

        Ok(())
    }

    async fn delete_identity(&self, id: LazySnowflake) -> IdentityResult<()> {
        let query = sqlx::query("DELETE FROM identity WHERE id = $1").bind(*id as i64);

        query.execute(&self.pool).await?;

        Ok(())
    }

    async fn find_identity(&self, id: LazySnowflake) -> IdentityResult<Option<Identity>> {
        let query = sqlx::query("SELECT * FROM identity WHERE id = $1").bind(*id as i64);

        if let Some(item) = query.fetch_optional(&self.pool).await? {
            let id: i64 = item.try_get("id")?;
            let id = id as u64;

            let ident = Identity {
                id: id.into(),
                name: item.try_get("name")?,
            };
            Ok(Some(ident))
        } else {
            Ok(None)
        }
    }
}
