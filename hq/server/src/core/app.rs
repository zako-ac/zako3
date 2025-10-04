use crate::infra::postgres::PostgresDb;

#[derive(Clone, Debug)]
pub struct AppState {
    pub db: PostgresDb,
}
