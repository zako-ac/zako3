pub mod hub;

pub use hub::{HubHandler, ZakofishHub};
pub use zakofish::config::create_server_config;
pub use zakofish::error::{Result, ZakofishError};

// Re-export shared protocol types so callers only need zakofish-taphub
pub use zakofish::types;
