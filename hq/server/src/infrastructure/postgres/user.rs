use crate::{
    feature::user::{
        User,
        error::UserError,
        repository::{UpdateUser, UserRepository},
    },
    infrastructure::postgres::PostgresDb,
    util::{
        error::{AppError, AppResult, BusinessError},
        permission::PermissionFlags,
        snowflake::LazySnowflake,
    },
};

use async_trait::async_trait;
use sqlx::{QueryBuilder, Row, postgres::PgDatabaseError};

fn map_user_query_error(err: sqlx::Error) -> AppError {
    if let Some(db_err) = err.as_database_error() {
        if let Some(pg_err) = db_err.try_downcast_ref::<PgDatabaseError>() {
            // <table>_<field>_key
            if pg_err.code() == "23505" && pg_err.constraint() == Some("users_name_key") {
                return AppError::Business(BusinessError::User(UserError::DuplicateName));
            }
        }
    }
    err.into()
}

#[async_trait]
impl UserRepository for PostgresDb {
    async fn create_user(&self, data: User) -> AppResult<()> {
        let query = sqlx::query("INSERT INTO users (id, name, permissions) VALUES ($1, $2, $3)")
            .bind(*data.id as i64)
            .bind(&data.name)
            .bind(data.permissions.bits() as i64);

        query
            .execute(&self.pool)
            .await
            .map_err(map_user_query_error)?;

        Ok(())
    }

    async fn update_user(&self, id: LazySnowflake, data: UpdateUser) -> AppResult<()> {
        let mut qb = QueryBuilder::new("UPDATE users SET ");

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
            let result = query
                .execute(&self.pool)
                .await
                .map_err(map_user_query_error)?;

            if result.rows_affected() > 0 {
                Ok(())
            } else {
                Err(AppError::NotFound)
            }
        } else {
            Ok(())
        }
    }

    async fn delete_user(&self, id: LazySnowflake) -> AppResult<()> {
        let query = sqlx::query("DELETE FROM users WHERE id = $1").bind(*id as i64);

        let result = query.execute(&self.pool).await?;

        if result.rows_affected() > 0 {
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }

    async fn find_user(&self, id: LazySnowflake) -> AppResult<Option<User>> {
        let query = sqlx::query("SELECT * FROM users WHERE id = $1").bind(*id as i64);

        if let Some(item) = query.fetch_optional(&self.pool).await? {
            let id: i64 = item.try_get("id")?;
            let id = id as u64;

            let permission_num: i64 = item.try_get("permissions")?;

            let ident = User {
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

