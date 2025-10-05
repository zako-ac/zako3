use testcontainers_modules::{
    postgres::{self, Postgres},
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use zako3_hq_server::infrastructure::postgres::{PostgresDb, connect_postgres, migrate_postgres};

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

pub async fn create_postgres_test() -> (ContainerAsync<Postgres>, PostgresDb) {
    let (handle, conn) = make_postgres_container().await;
    let db = connect_postgres(&conn).await.unwrap();

    migrate_postgres(&db).await.unwrap();

    (handle, db)
}
