use parking_lot::Mutex;
use snowflake::SnowflakeIdGenerator;

lazy_static::lazy_static! {
    static ref SNOWFLAKE_GEN: Mutex<SnowflakeIdGenerator> = SnowflakeIdGenerator::new(1, 1).into();
}

pub fn generate_id<T>() -> T
where
    T: From<u64>,
{
    (SNOWFLAKE_GEN.lock().generate() as u64).into()
}
