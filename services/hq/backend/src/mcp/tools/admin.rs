//! Admin tools (mirrors `handlers::admin`, plus admin tap + cache ops). All require an admin user.

use crate::handlers::cache::DeleteCacheEntryDto;
use crate::mcp::auth::require_admin;
use crate::mcp::support::{json_ok, map_core, mk_tool, parse_args, run, text_ok};
use hq_core::Service;
use hq_types::cache::AudioCacheItemKey;
use hq_types::hq::settings::PartialUserSettings;
use hq_types::hq::{
    AuthUserDto, PaginatedResponseDto, PlatformStatsDto, RejectVerificationDto, TapId,
    UpdateOccupationDto, UpdateTapDto, UpdateUserRoleDto, UserId, VerificationRequestId,
    VerificationStatus,
};
use mcpkit::server::capability::tools::ToolService;
use mcpkit::types::ToolOutput;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Deserialize)]
struct Page {
    page: Option<u32>,
    per_page: Option<u32>,
}

#[derive(Deserialize)]
struct UserRef {
    id: String,
}

#[derive(Deserialize)]
struct RoleArgs {
    id: String,
    #[serde(flatten)]
    body: UpdateUserRoleDto,
}

#[derive(Deserialize)]
struct VerificationsQuery {
    status: Option<VerificationStatus>,
    page: Option<u32>,
    per_page: Option<u32>,
}

#[derive(Deserialize)]
struct VerificationRef {
    id: VerificationRequestId,
}

#[derive(Deserialize)]
struct RejectArgs {
    id: VerificationRequestId,
    #[serde(flatten)]
    body: RejectVerificationDto,
}

#[derive(Deserialize)]
struct UserSettingsArgs {
    id: String,
    #[serde(flatten)]
    settings: PartialUserSettings,
}

#[derive(Deserialize)]
struct UserGuildArgs {
    id: String,
    guild_id: String,
}

#[derive(Deserialize)]
struct UserGuildSettingsArgs {
    id: String,
    guild_id: String,
    #[serde(flatten)]
    settings: PartialUserSettings,
}

#[derive(Deserialize)]
struct AdminTapUpdate {
    tap_id: TapId,
    #[serde(flatten)]
    body: UpdateTapDto,
}

#[derive(Deserialize)]
struct AdminOccupationUpdate {
    tap_id: TapId,
    #[serde(flatten)]
    body: UpdateOccupationDto,
}

#[derive(Deserialize)]
struct TapIdStr {
    tap_id: String,
}

#[derive(Deserialize)]
struct CacheEntryArgs {
    tap_id: String,
    #[serde(flatten)]
    body: DeleteCacheEntryDto,
}

fn parse_user_id(id: &str) -> Result<UserId, ToolOutput> {
    UserId::from_str(id).map_err(|_| ToolOutput::error("Invalid user ID"))
}

pub fn register(tools: &mut ToolService, service: &Arc<Service>) {
    let svc = service.clone();
    tools.register(
        mk_tool("admin_list_users", "Admin: list users (paginated).", json!({"type": "object", "properties": {"page": {"type": "integer"}, "per_page": {"type": "integer"}}})),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let Page { page, per_page } = parse_args(args)?;
                let page = page.unwrap_or(1);
                let per_page = per_page.unwrap_or(20);
                let (users, total) = svc.auth.list_all_users(page, per_page).await.map_err(map_core)?;
                let data: Vec<AuthUserDto> = users
                    .into_iter()
                    .map(|user| AuthUserDto {
                        id: user.id.0.clone(),
                        discord_id: user.discord_user_id.0.clone(),
                        username: user.username.0.clone(),
                        avatar: user.avatar_url.unwrap_or_default(),
                        email: user.email.clone(),
                        is_admin: user.permissions.contains(&"admin".to_string()),
                        banned: user.banned,
                    })
                    .collect();
                let result = PaginatedResponseDto {
                    data,
                    meta: hq_types::hq::dtos::PaginationMetaDto {
                        total,
                        page: page.into(),
                        per_page: per_page.into(),
                        total_pages: (total as f64 / per_page as f64).ceil() as u64,
                    },
                };
                json_ok(&result)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "admin_get_user",
            "Admin: get a user by id.",
            json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let UserRef { id } = parse_args(args)?;
                let user = svc.auth.get_user(&id).await.map_err(map_core)?;
                json_ok(&user)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "admin_ban_user",
            "Admin: ban a user.",
            json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let UserRef { id } = parse_args(args)?;
                let user = svc
                    .auth
                    .ban_user(parse_user_id(&id)?)
                    .await
                    .map_err(map_core)?;
                json_ok(&AuthUserDto {
                    id: user.id.0.clone(),
                    discord_id: user.discord_user_id.0.clone(),
                    username: user.username.0.clone(),
                    avatar: user.avatar_url.unwrap_or_default(),
                    email: user.email.clone(),
                    is_admin: user.permissions.contains(&"admin".to_string()),
                    banned: user.banned,
                })
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "admin_unban_user",
            "Admin: unban a user.",
            json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let UserRef { id } = parse_args(args)?;
                let user = svc
                    .auth
                    .unban_user(parse_user_id(&id)?)
                    .await
                    .map_err(map_core)?;
                json_ok(&AuthUserDto {
                    id: user.id.0.clone(),
                    discord_id: user.discord_user_id.0.clone(),
                    username: user.username.0.clone(),
                    avatar: user.avatar_url.unwrap_or_default(),
                    email: user.email.clone(),
                    is_admin: user.permissions.contains(&"admin".to_string()),
                    banned: user.banned,
                })
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool("admin_update_user_role", "Admin: set a user's role ('admin' or 'user').", json!({"type": "object", "properties": {"id": {"type": "string"}, "role": {"type": "string"}}, "required": ["id", "role"]})),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let RoleArgs { id, body } = parse_args(args)?;
                let user_id = parse_user_id(&id)?;
                let mut permissions = vec![];
                if body.role == "admin" {
                    permissions.push("admin".to_string());
                }
                let user = svc
                    .auth
                    .update_user_permissions(user_id, permissions)
                    .await
                    .map_err(map_core)?;
                json_ok(&AuthUserDto {
                    id: user.id.0.clone(),
                    discord_id: user.discord_user_id.0.clone(),
                    username: user.username.0.clone(),
                    avatar: user.avatar_url.unwrap_or_default(),
                    email: user.email.clone(),
                    is_admin: user.permissions.contains(&"admin".to_string()),
                    banned: user.banned,
                })
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool("admin_list_verification_requests", "Admin: list tap verification requests.", json!({"type": "object", "properties": {"status": {"type": "string"}, "page": {"type": "integer"}, "per_page": {"type": "integer"}}})),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let VerificationsQuery { status, page, per_page } = parse_args(args)?;
                let page = page.unwrap_or(1);
                let per_page = per_page.unwrap_or(20);
                let (requests, total) = svc
                    .verification
                    .list_requests(status, page, per_page)
                    .await
                    .map_err(map_core)?;
                let result = json!({
                    "data": requests,
                    "meta": {
                        "total": total,
                        "page": page,
                        "per_page": per_page,
                        "total_pages": (total as f64 / per_page as f64).ceil() as u64,
                    }
                });
                json_ok(&result)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "admin_approve_verification",
            "Admin: approve a verification request.",
            json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let admin_id = require_admin()?;
                let VerificationRef { id } = parse_args(args)?;
                let request = svc
                    .verification
                    .approve_verification(id, admin_id)
                    .await
                    .map_err(map_core)?;
                json_ok(&request)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool("admin_reject_verification", "Admin: reject a verification request. Provide id and reason.", json!({"type": "object", "properties": {"id": {"type": "string"}, "reason": {"type": "string"}}, "required": ["id"]})),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let admin_id = require_admin()?;
                let RejectArgs { id, body } = parse_args(args)?;
                let request = svc
                    .verification
                    .reject_verification(id, admin_id, body.reason)
                    .await
                    .map_err(map_core)?;
                json_ok(&request)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "admin_get_user_settings",
            "Admin: get a user's settings (User scope).",
            json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let UserRef { id } = parse_args(args)?;
                let settings = svc
                    .user_settings
                    .get_settings(parse_user_id(&id)?)
                    .await
                    .map_err(map_core)?;
                json_ok(&settings)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "admin_update_user_settings",
            "Admin: update a user's settings. Provide id plus PartialUserSettings fields.",
            json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let UserSettingsArgs { id, settings } = parse_args(args)?;
                let saved = svc
                    .user_settings
                    .save_settings(parse_user_id(&id)?, settings)
                    .await
                    .map_err(map_core)?;
                json_ok(&saved)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool("admin_get_user_guild_settings", "Admin: get a user's per-guild settings override.", json!({"type": "object", "properties": {"id": {"type": "string"}, "guild_id": {"type": "string"}}, "required": ["id", "guild_id"]})),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let UserGuildArgs { id, guild_id } = parse_args(args)?;
                let settings = svc
                    .user_settings
                    .get_guild_user_settings(&parse_user_id(&id)?, &guild_id)
                    .await
                    .map_err(map_core)?
                    .unwrap_or_default();
                json_ok(&settings)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool("admin_update_user_guild_settings", "Admin: update a user's per-guild settings override.", json!({"type": "object", "properties": {"id": {"type": "string"}, "guild_id": {"type": "string"}}, "required": ["id", "guild_id"]})),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let UserGuildSettingsArgs { id, guild_id, settings } = parse_args(args)?;
                let saved = svc
                    .user_settings
                    .save_guild_user_settings(&parse_user_id(&id)?, &guild_id, settings)
                    .await
                    .map_err(map_core)?;
                json_ok(&saved)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "admin_get_platform_stats",
            "Admin: get platform-wide stats.",
            json!({"type": "object"}),
        ),
        move |_args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let global_unique_users = svc
                    .tap_metrics
                    .get_global_unique_users()
                    .await
                    .map_err(|e| ToolOutput::error(e.to_string()))?;
                json_ok(&PlatformStatsDto {
                    global_unique_users,
                })
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool("admin_update_tap", "Admin: update any tap. Provide tap_id plus UpdateTapDto fields.", json!({"type": "object", "properties": {"tap_id": {"type": "string"}}, "required": ["tap_id"]})),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let admin_id = require_admin()?;
                let AdminTapUpdate { tap_id, body } = parse_args(args)?;
                svc.tap.admin_update_tap(tap_id, admin_id, body).await.map_err(map_core)?;
                text_ok("updated")
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool("admin_update_tap_occupation", "Admin: update a tap's occupation. Provide tap_id plus UpdateOccupationDto fields.", json!({"type": "object", "properties": {"tap_id": {"type": "string"}}, "required": ["tap_id"]})),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let admin_id = require_admin()?;
                let AdminOccupationUpdate { tap_id, body } = parse_args(args)?;
                svc.tap.admin_update_occupation(tap_id, admin_id, body).await.map_err(map_core)?;
                text_ok("updated")
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool("admin_clear_tap_cache", "Admin: clear all cached audio for a tap.", json!({"type": "object", "properties": {"tap_id": {"type": "string"}}, "required": ["tap_id"]})),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let TapIdStr { tap_id } = parse_args(args)?;
                let deleted = svc
                    .cache_admin
                    .delete_all_for_tap(&TapId(tap_id))
                    .await
                    .map_err(|e| ToolOutput::error(e.to_string()))?;
                json_ok(&json!({"deleted": deleted}))
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool("admin_delete_tap_cache_entry", "Admin: delete one cached audio entry. Provide tap_id and either audio_request or cache_key.", json!({"type": "object", "properties": {"tap_id": {"type": "string"}, "audio_request": {"type": "string"}, "cache_key": {"type": "string"}}, "required": ["tap_id"]})),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let CacheEntryArgs { tap_id, body } = parse_args(args)?;
                let key = if let Some(k) = body.cache_key.filter(|s| !s.trim().is_empty()) {
                    AudioCacheItemKey::CacheKey(k)
                } else if let Some(req) = body.audio_request.filter(|s| !s.is_empty()) {
                    AudioCacheItemKey::ARHash(hex::encode(Sha256::digest(req.as_bytes())))
                } else {
                    return Err(ToolOutput::error("either audio_request or cache_key is required"));
                };
                let found = svc
                    .cache_admin
                    .delete_entry(&TapId(tap_id), &key)
                    .await
                    .map_err(|e| ToolOutput::error(e.to_string()))?;
                json_ok(&json!({"found": found}))
            })
        },
    );
}
