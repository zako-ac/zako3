use testcontainers_modules::{
    redis::Redis,
    testcontainers::{ContainerAsync, runners::AsyncRunner},
};
use zako3_hq_server::infrastructure::redis::RedisDb;

async fn make_redis_container() -> (ContainerAsync<Redis>, String) {
    let container = testcontainers_modules::redis::Redis::default()
        .start()
        .await
        .unwrap();

    let host = container.get_host().await.unwrap();
    let host_port = container.get_host_port_ipv4(6379).await.unwrap();
    let connection_string = format!("redis://{host}:{host_port}");

    (container, connection_string)
}

pub async fn create_redis_test() -> (ContainerAsync<Redis>, RedisDb) {
    let (container, connection_str) = make_redis_container().await;

    let redis_db = RedisDb::connect(&connection_str).await.unwrap();

    (container, redis_db)
}
