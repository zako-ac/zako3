use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Field<T> {
    pub important: bool,
    pub value: Option<T>,
}

impl<T> Field<T> {
    pub fn new_important(value: T) -> Self {
        Self {
            important: true,
            value: Some(value),
        }
    }

    pub fn new_non_important(value: T) -> Self {
        Self {
            important: false,
            value: Some(value),
        }
    }

    pub fn new_default() -> Self {
        Self {
            important: false,
            value: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SettingsScope {
    User(u64),
    Guild(String),
    Global,
}

impl SettingsScope {
    pub fn as_key(&self) -> String {
        match self {
            SettingsScope::User(user_id) => format!("USER:{user_id}"),
            SettingsScope::Guild(guild_id) => format!("GUILD:{guild_id}"),
            SettingsScope::Global => "GLOBAL".to_string(),
        }
    }
}

impl<T> Default for Field<T> {
    fn default() -> Self {
        Self {
            important: false,
            value: None,
        }
    }
}
