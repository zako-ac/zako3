use crate::infrastructure::postgres::PostgresDb;

#[derive(Clone, Debug)]
pub struct AppState {
    pub db: PostgresDb,
}
