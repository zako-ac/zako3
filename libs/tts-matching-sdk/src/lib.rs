//! Zako3 TTS Matching SDK
//!
//! Ergonomic Rust library for writing WASM text-transformation mappers for Zako3.
//!
//! ## Example
//!
//! ```ignore
//! use zako3_tts_matching_sdk::prelude::*;
//!
//! fn process(input: Input) -> Output {
//!     let mut text = input.text.clone();
//!
//!     // Replace common abbreviations
//!     text = text.replace("btw", "by the way");
//!     text = text.replace("imo", "in my opinion");
//!
//!     // Optionally look up Discord info
//!     if let Some(ch) = input.query_channel() {
//!         eprintln!("Channel: {}", ch.name);
//!     }
//!
//!     Output::text(text)
//! }
//!
//! export_mapper!(process);
//! ```
//!
//! Compile with:
//! ```sh
//! cargo build --target wasm32-unknown-unknown --release
//! ```

pub mod host;
pub mod macros;
pub mod types;

// Private module for macro's use — allows macros to access crate dependencies
pub mod __private {
    pub use serde_json;
}

/// Convenient prelude for mapper implementations
pub mod prelude {
    pub use crate::types::{
        CallerInfo, ChannelInfo, EmojiMappingRule, Input, MapperList, MapperSummary, MappingList,
        Output, TextMappingRule, UserInfo,
    };
    pub use crate::export_mapper;
    pub use crate::host::{query_channel_raw, query_user_raw};
}
