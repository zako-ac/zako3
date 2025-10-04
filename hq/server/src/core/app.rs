use crate::infra::postgres::PostgresDb;

pub struct AppState {
    pub db: PostgresDb,
}
