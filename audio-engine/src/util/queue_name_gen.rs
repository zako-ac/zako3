use crate::types::{QueueName, UserId};

pub fn music() -> QueueName {
    "music".to_string().into()
}

pub fn tts(user_id: UserId) -> QueueName {
    format!("tts_{}", u64::from(user_id)).into()
}
