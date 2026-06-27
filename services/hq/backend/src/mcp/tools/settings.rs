//! Guild- and global-scope settings tools (mirrors `handlers::settings`).

use crate::mcp::auth::require_user;
use crate::mcp::support::{json_ok, map_core, mk_tool, parse_args, run};
use hq_core::Service;
use hq_types::hq::settings::PartialUserSettings;
use mcpkit::server::capability::tools::ToolService;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Deserialize)]
struct GuildId {
    guild_id: String,
}

#[derive(Deserialize)]
struct GuildSettingsArgs {
    guild_id: String,
    #[serde(flatten)]
    settings: PartialUserSettings,
}

pub fn register(tools: &mut ToolService, service: &Arc<Service>) {
    let svc = service.clone();
    tools.register(
        mk_tool(
            "get_guild_settings",
            "Get guild-wide settings.",
            json!({"type": "object", "properties": {"guild_id": {"type": "string"}}, "required": ["guild_id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_user()?;
                let GuildId { guild_id } = parse_args(args)?;
                let settings = svc
                    .user_settings
                    .get_guild_settings(&guild_id)
                    .await
                    .map_err(map_core)?
                    .unwrap_or_default();
                json_ok(&settings)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "update_guild_settings",
            "Update guild-wide settings. Provide guild_id plus PartialUserSettings fields.",
            json!({"type": "object", "properties": {"guild_id": {"type": "string"}}, "required": ["guild_id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_user()?;
                let GuildSettingsArgs { guild_id, settings } = parse_args(args)?;
                let saved = svc
                    .user_settings
                    .save_guild_settings(&guild_id, settings)
                    .await
                    .map_err(map_core)?;
                json_ok(&saved)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "get_global_settings",
            "Get the global settings baseline.",
            json!({"type": "object"}),
        ),
        move |_args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_user()?;
                let settings = svc
                    .user_settings
                    .get_global_settings()
                    .await
                    .map_err(map_core)?
                    .unwrap_or_default();
                json_ok(&settings)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "update_global_settings",
            "Update the global settings baseline. Body is a PartialUserSettings object.",
            json!({"type": "object"}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_user()?;
                let body: PartialUserSettings = parse_args(args)?;
                let saved = svc
                    .user_settings
                    .save_global_settings(body)
                    .await
                    .map_err(map_core)?;
                json_ok(&saved)
            })
        },
    );
}
