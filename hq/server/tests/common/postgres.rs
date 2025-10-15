use testcontainers_modules::{
    postgres::{self, Postgres},
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use tokio::sync::OnceCell;
use zako3_hq_server::infrastructure::postgres::{PostgresDb, connect_postgres, migrate_postgres};

static DB_CONTAINER: OnceCell<(ContainerAsync<Postgres>, String)> = OnceCell::const_new();

async fn make_postgres_container() -> (ContainerAsync<Postgres>, String) {
    let container = postgres::Postgres::default()
        .with_env_var("POSTGRES_HOST_AUTH_METHOD".to_string(), "trust".to_string())
        .start()
        .await
        .unwrap();
    let host = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(5432).await.unwrap();
    let connection_string = format!("postgres://postgres:postgres@{host}:{host_port}/postgres");

    (container, connection_string)
}

async fn create_postgres_test() -> (ContainerAsync<Postgres>, String) {
    let (handle, conn) = make_postgres_container().await;
    let db = connect_postgres(&conn).await.unwrap();

    migrate_postgres(&db).await.unwrap();

    (handle, conn)
}

pub async fn init_postgres() -> PostgresDb {
    DB_CONTAINER
        .get_or_init(|| async { create_postgres_test().await })
        .await;

    let url = DB_CONTAINER.get().unwrap().1.clone();

    connect_postgres(&url).await.unwrap()
}
