use crate::types::{QueueName, UserId};

pub fn music() -> QueueName {
    "music".to_string().into()
}

pub fn tts(user_id: UserId) -> QueueName {
    format!("tts_{}", user_id.0).into()
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::types::UserId;

    #[test]
    fn test_music_queue_name() {
        assert_eq!(music(), QueueName::from("music".to_string()));
    }

    #[test]
    fn test_tts_queue_name() {
        let user_id = UserId::from_str("123456789").unwrap();
        assert_eq!(tts(user_id), QueueName::from("tts_123456789".to_string()));
    }
}
