use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_current_context")]
    pub current_context: String,
    #[serde(default)]
    pub contexts: Vec<Context>,
    #[serde(default)]
    pub aliases: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Context {
    pub name: String,
    pub ae_addr: String,
    #[serde(default)]
    pub default_guild_id: Option<String>,
}

fn default_current_context() -> String {
    "default".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            current_context: "default".to_string(),
            contexts: vec![Context {
                name: "default".to_string(),
                ae_addr: "http://127.0.0.1:50051".to_string(),
                default_guild_id: None,
            }],
            aliases: HashMap::new(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = get_config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)
            .context(format!("Failed to read config file at {:?}", path))?;

        let config: Config = toml::from_str(&content).context("Failed to parse config file")?;

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = get_config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn get_active_context(&self) -> Option<&Context> {
        self.contexts
            .iter()
            .find(|c| c.name == self.current_context)
    }

    pub fn get_context(&self, name: &str) -> Option<&Context> {
        self.contexts.iter().find(|c| c.name == name)
    }

    pub fn set_context(&mut self, name: String, ae_addr: String, default_guild_id: Option<String>) {
        if let Some(ctx) = self.contexts.iter_mut().find(|c| c.name == name) {
            ctx.ae_addr = ae_addr;
            ctx.default_guild_id = default_guild_id;
        } else {
            self.contexts.push(Context {
                name,
                ae_addr,
                default_guild_id,
            });
        }
    }

    pub fn set_alias(&mut self, key: String, value: String) {
        self.aliases.insert(key, value);
    }

    pub fn get_alias(&self, key: &str) -> Option<&String> {
        self.aliases.get(key)
    }

    pub fn delete_alias(&mut self, key: &str) -> Option<String> {
        self.aliases.remove(key)
    }

    pub fn resolve_alias(&self, input: &str) -> String {
        self.aliases
            .get(input)
            .cloned()
            .unwrap_or_else(|| input.to_string())
    }
}

pub fn get_config_path() -> Result<PathBuf> {
    let mut path = dirs::config_dir().context("Could not find config directory")?;
    path.push("zakoctl");
    path.push("config.toml");
    Ok(path)
}
