use crate::{
    feature::identity::{
        entity::Identity,
        error::{IdentityError, IdentityResult},
        repository::{IdentityRepository, UpdateIdentity},
    },
    infrastructure::postgres::PostgresDb,
    util::{permission::PermissionFlags, snowflake::LazySnowflake},
};

use async_trait::async_trait;
use sqlx::{QueryBuilder, Row};

#[async_trait]
impl IdentityRepository for PostgresDb {
    async fn create_identity(&self, data: &Identity) -> IdentityResult<()> {
        let query = sqlx::query("INSERT INTO identity (id, name, permissions) VALUES ($1, $2, $3)")
            .bind(*data.id as i64)
            .bind(&data.name)
            .bind(data.permissions.bits() as i64);

        query.execute(&self.pool).await?;

        Ok(())
    }

    async fn update_identity(
        &self,
        id: LazySnowflake,
        data: &UpdateIdentity,
    ) -> Result<(), IdentityError> {
        let mut qb = QueryBuilder::new("UPDATE identity SET ");

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
            qb.push("name = ").push_bind(name);
        }
        if let Some(ref permissions) = data.permissions {
            handle_sep!();
            qb.push("permissions = ")
                .push_bind(permissions.bits() as i64);
        }

        if separated {
            qb.push(" WHERE id = ").push_bind(*id as i64);

            let query = qb.build();
            query.execute(&self.pool).await?;
        }

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

            let permission_num: i64 = item.try_get("permissions")?;

            let ident = Identity {
                id: id.into(),
                name: item.try_get("name")?,
                permissions: PermissionFlags::from_bits_retain(permission_num as u32),
            };
            Ok(Some(ident))
        } else {
            Ok(None)
        }
    }
}
