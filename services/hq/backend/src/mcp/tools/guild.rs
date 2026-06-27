//! Guild tools (mirrors `handlers::guild`).

use crate::mcp::auth::{require_admin, require_user};
use crate::mcp::support::{json_ok, map_core, mk_tool, parse_args, run};
use hq_core::Service;
use hq_types::hq::guild::GuildSummaryDto;
use mcpkit::server::capability::tools::ToolService;
use mcpkit::types::ToolOutput;
use serde::Deserialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Deserialize)]
struct UserRef {
    id: String,
}

pub fn register(tools: &mut ToolService, service: &Arc<Service>) {
    let svc = service.clone();
    tools.register(
        mk_tool(
            "get_my_guilds",
            "List guilds where the current user is a member and the bot is present.",
            json!({"type": "object"}),
        ),
        move |_args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let db_user = svc
                    .auth
                    .get_full_user(&uid.to_string())
                    .await
                    .map_err(map_core)?;
                let discord_id_str = db_user.discord_user_id.0.clone();
                let discord_id: u64 = discord_id_str
                    .parse()
                    .map_err(|_| ToolOutput::error("invalid discord id"))?;

                let voice_channels = svc
                    .voice_state
                    .get_user_channels(&discord_id_str)
                    .await
                    .map_err(|e| ToolOutput::error(e.to_string()))?;
                let voice_map: HashMap<u64, _> = voice_channels
                    .into_iter()
                    .map(|loc| (loc.guild_id, loc))
                    .collect();

                let guild_infos = if let Some(token) = &db_user.oauth_access_token {
                    match svc
                        .auth
                        .fetch_discord_guilds_for_user(&discord_id_str, token)
                        .await
                    {
                        Ok(guilds) => guilds,
                        Err(_) => svc
                            .name_resolver_slot
                            .get()
                            .map(|r| r.guilds_for_user(discord_id))
                            .unwrap_or_default(),
                    }
                } else {
                    svc.name_resolver_slot
                        .get()
                        .map(|r| r.guilds_for_user(discord_id))
                        .unwrap_or_default()
                };

                let resolver = svc.name_resolver_slot.get();
                let bot_guild_ids: HashSet<u64> = resolver
                    .map(|r| r.bot_guilds().into_iter().collect())
                    .unwrap_or_default();

                let dtos: Vec<GuildSummaryDto> = guild_infos
                    .into_iter()
                    .filter(|g| bot_guild_ids.contains(&g.id))
                    .map(|g| {
                        let vc = voice_map.get(&g.id);
                        GuildSummaryDto {
                            guild_id: g.id.to_string(),
                            guild_name: g.name,
                            guild_icon_url: g.icon_url,
                            active_channel_id: vc.map(|v| v.channel_id.to_string()),
                            active_channel_name: vc.map(|v| v.channel_name.clone()),
                            can_manage: g.can_manage,
                        }
                    })
                    .collect();
                json_ok(&dtos)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "admin_get_user_guilds",
            "Admin: list guilds where a user is a member and the bot is present.",
            json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let UserRef { id } = parse_args(args)?;
                let db_user = svc.auth.get_full_user(&id).await.map_err(map_core)?;
                let discord_id_str = db_user.discord_user_id.0.clone();
                let discord_id: u64 = discord_id_str
                    .parse()
                    .map_err(|_| ToolOutput::error("invalid discord id"))?;

                let resolver = svc.name_resolver_slot.get();
                let guild_infos = resolver
                    .as_ref()
                    .map(|r| r.guilds_for_user(discord_id))
                    .unwrap_or_default();
                let bot_guild_ids: HashSet<u64> = resolver
                    .map(|r| r.bot_guilds().into_iter().collect())
                    .unwrap_or_default();

                let dtos: Vec<GuildSummaryDto> = guild_infos
                    .into_iter()
                    .filter(|g| bot_guild_ids.contains(&g.id))
                    .map(|g| GuildSummaryDto {
                        guild_id: g.id.to_string(),
                        guild_name: g.name,
                        guild_icon_url: g.icon_url,
                        active_channel_id: None,
                        active_channel_name: None,
                        can_manage: g.can_manage,
                    })
                    .collect();
                json_ok(&dtos)
            })
        },
    );
}
