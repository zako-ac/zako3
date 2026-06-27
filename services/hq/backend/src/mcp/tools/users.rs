//! Current-user tools (mirrors `handlers::users`). All require an authenticated user.

use crate::mcp::auth::require_user;
use crate::mcp::support::{json_ok, map_core, mk_tool, parse_args, run, text_ok};
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

#[derive(Deserialize)]
struct EffectiveArgs {
    guild_id: Option<String>,
}

pub fn register(tools: &mut ToolService, service: &Arc<Service>) {
    let svc = service.clone();
    tools.register(
        mk_tool(
            "get_me",
            "Get the current user's profile",
            json!({"type": "object"}),
        ),
        move |_args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let user = svc
                    .auth
                    .get_user(&uid.to_string())
                    .await
                    .map_err(map_core)?;
                json_ok(&user)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "get_my_taps",
            "List taps owned by the current user",
            json!({"type": "object"}),
        ),
        move |_args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let taps = svc.tap.list_by_user(uid).await.map_err(map_core)?;
                json_ok(&taps)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "get_my_settings",
            "Get the current user's settings (User scope)",
            json!({"type": "object"}),
        ),
        move |_args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let settings = svc
                    .user_settings
                    .get_settings(uid)
                    .await
                    .map_err(map_core)?;
                json_ok(&settings)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "update_my_settings",
            "Update the current user's settings (User scope). Body is a PartialUserSettings object.",
            json!({"type": "object"}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let body: PartialUserSettings = parse_args(args)?;
                let settings = svc.user_settings.save_settings(uid, body).await.map_err(map_core)?;
                json_ok(&settings)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "get_my_guild_settings",
            "Get the current user's per-guild settings override",
            json!({"type": "object", "properties": {"guild_id": {"type": "string"}}, "required": ["guild_id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let GuildId { guild_id } = parse_args(args)?;
                let settings = svc
                    .user_settings
                    .get_guild_user_settings(&uid, &guild_id)
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
            "update_my_guild_settings",
            "Update the current user's per-guild settings override. Provide guild_id plus PartialUserSettings fields.",
            json!({"type": "object", "properties": {"guild_id": {"type": "string"}}, "required": ["guild_id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let GuildSettingsArgs { guild_id, settings } = parse_args(args)?;
                let saved = svc
                    .user_settings
                    .save_guild_user_settings(&uid, &guild_id, settings)
                    .await
                    .map_err(map_core)?;
                json_ok(&saved)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "delete_my_guild_settings",
            "Delete the current user's per-guild settings override",
            json!({"type": "object", "properties": {"guild_id": {"type": "string"}}, "required": ["guild_id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let GuildId { guild_id } = parse_args(args)?;
                svc.user_settings
                    .delete_guild_user_settings(&uid, &guild_id)
                    .await
                    .map_err(map_core)?;
                text_ok("deleted")
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "get_effective_settings",
            "Get fully-resolved settings (optionally for a specific guild)",
            json!({"type": "object", "properties": {"guild_id": {"type": "string"}}}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let EffectiveArgs { guild_id } = parse_args(args)?;
                let settings = svc
                    .user_settings
                    .get_effective_settings(&Some(uid), guild_id.as_deref())
                    .await
                    .map_err(map_core)?;
                json_ok(&settings)
            })
        },
    );
}
