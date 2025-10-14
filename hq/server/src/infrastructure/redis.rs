use redis::{Client, RedisError, aio::ConnectionManager};

pub mod token;

pub struct RedisDb {
    connection_manager: ConnectionManager,
}

impl RedisDb {
    pub async fn connect(url: &str) -> Result<Self, RedisError> {
        let client = Client::open(url)?;
        let connection_manager = client.get_connection_manager().await?;
        Ok(Self { connection_manager })
    }

    pub fn connection_manager(&self) -> ConnectionManager {
        self.connection_manager.clone()
    }
}
