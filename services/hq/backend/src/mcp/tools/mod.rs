//! Tool registration. Each submodule mirrors a handler group, registering one
//! tool per REST operation with the same service call, DTO and auth tier.

use hq_core::Service;
use mcpkit::server::capability::tools::ToolService;
use std::sync::Arc;

mod admin;
mod api_key;
mod guild;
mod mapper;
mod notification;
mod playback;
mod settings;
mod taps;
mod users;

/// Register every HQ API tool onto the given `ToolService`.
pub fn register_all(tools: &mut ToolService, service: &Arc<Service>) {
    users::register(tools, service);
    taps::register(tools, service);
    api_key::register(tools, service);
    settings::register(tools, service);
    guild::register(tools, service);
    notification::register(tools, service);
    playback::register(tools, service);
    admin::register(tools, service);
    mapper::register(tools, service);
}
