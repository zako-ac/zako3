//! Tap tools (mirrors `handlers::tap` + `handlers::audit_log`).

use crate::mcp::auth::{optional_user, require_user};
use crate::mcp::support::{json_ok, map_core, mk_tool, parse_args, run, text_ok};
use hq_core::{Service, SortDirection, TapSortField};
use hq_types::hq::{
    CreateTapDto, CreateVerificationRequestDto, TapDto, TapId, TapStatsDto, UpdateTapDto,
};
use mcpkit::server::capability::tools::ToolService;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Deserialize)]
struct TapRef {
    tap_id: TapId,
}

#[derive(Deserialize)]
struct UpdateTapArgs {
    tap_id: TapId,
    #[serde(flatten)]
    body: UpdateTapDto,
}

#[derive(Deserialize)]
struct VerifyArgs {
    tap_id: TapId,
    #[serde(flatten)]
    body: CreateVerificationRequestDto,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ListTapsArgs {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    roles: Option<String>,
    accessible: Option<bool>,
    sort_field: Option<TapSortField>,
    sort_direction: Option<SortDirection>,
}

#[derive(Deserialize)]
struct AuditLogArgs {
    tap_id: TapId,
    page: Option<i64>,
    limit: Option<i64>,
}

pub fn register(tools: &mut ToolService, service: &Arc<Service>) {
    let svc = service.clone();
    tools.register(
        mk_tool(
            "create_tap",
            "Create a new tap. Body is a CreateTapDto.",
            json!({"type": "object"}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let payload: CreateTapDto = parse_args(args)?;
                let tap = svc.tap.create(uid, payload).await.map_err(map_core)?;
                let tap_id_str = tap.id.0.to_string();
                let dto = TapDto {
                    id: tap_id_str.clone(),
                    name: tap.name.0,
                    description: tap.description.unwrap_or_default(),
                    owner_id: tap.owner_id.0.to_string(),
                    occupation: tap.occupation,
                    permission: tap.permission,
                    roles: tap.roles,
                    base_volume: tap.base_volume,
                    total_uses: 0,
                    cache_hits: 0,
                    created_at: tap.timestamp.created_at,
                    updated_at: tap.timestamp.updated_at,
                    stats: TapStatsDto {
                        tap_id: tap_id_str,
                        currently_active: 0,
                        total_uses: 0,
                        cache_hits: 0,
                        unique_users: 0,
                        uptime_percent: 0.0,
                        use_rate_history: vec![],
                        cache_hit_rate_history: vec![],
                    },
                };
                json_ok(&dto)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "list_taps",
            "List taps (paginated, optionally filtered). Auth optional.",
            json!({"type": "object"}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let q: ListTapsArgs = parse_args(args)?;
                let taps = svc
                    .tap
                    .list_all_paginated(
                        optional_user(),
                        q.sort_field,
                        q.sort_direction,
                        q.search,
                        q.roles,
                        q.accessible,
                        q.page,
                        q.per_page,
                    )
                    .await
                    .map_err(map_core)?;
                json_ok(&taps)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "get_tap",
            "Get a tap by id (with access info). Auth optional.",
            json!({"type": "object", "properties": {"tap_id": {"type": "string"}}, "required": ["tap_id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let TapRef { tap_id } = parse_args(args)?;
                let tap = svc
                    .tap
                    .get_tap_with_access(tap_id, optional_user())
                    .await
                    .map_err(map_core)?;
                json_ok(&tap)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "update_tap",
            "Update a tap you own. Provide tap_id plus UpdateTapDto fields.",
            json!({"type": "object", "properties": {"tap_id": {"type": "string"}}, "required": ["tap_id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let UpdateTapArgs { tap_id, body } = parse_args(args)?;
                svc.tap.update_tap(tap_id, uid, body).await.map_err(map_core)?;
                text_ok("updated")
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "delete_tap",
            "Delete a tap you own.",
            json!({"type": "object", "properties": {"tap_id": {"type": "string"}}, "required": ["tap_id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let TapRef { tap_id } = parse_args(args)?;
                svc.tap.delete_tap(tap_id, uid).await.map_err(map_core)?;
                text_ok("deleted")
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "request_tap_verification",
            "Request verification for a tap. Provide tap_id plus CreateVerificationRequestDto fields.",
            json!({"type": "object", "properties": {"tap_id": {"type": "string"}}, "required": ["tap_id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let VerifyArgs { tap_id, body } = parse_args(args)?;
                let request = svc
                    .verification
                    .request_verification(tap_id, uid, body)
                    .await
                    .map_err(map_core)?;
                json_ok(&request)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "get_tap_stats",
            "Get statistics for a tap.",
            json!({"type": "object", "properties": {"tap_id": {"type": "string"}}, "required": ["tap_id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let TapRef { tap_id } = parse_args(args)?;
                let stats = svc.tap.get_tap_stats(tap_id, uid).await.map_err(map_core)?;
                json_ok(&stats)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "get_tap_audit_logs",
            "Get paginated audit logs for a tap you can access.",
            json!({"type": "object", "properties": {"tap_id": {"type": "string"}, "page": {"type": "integer"}, "limit": {"type": "integer"}}, "required": ["tap_id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let AuditLogArgs { tap_id, page, limit } = parse_args(args)?;
                let page = page.unwrap_or(1).max(1);
                let limit = limit.unwrap_or(50).clamp(1, 100);
                let tap = svc
                    .tap
                    .get_tap_with_access(tap_id.clone(), Some(uid))
                    .await
                    .map_err(map_core)?;
                if !tap.has_access {
                    return Err(mcpkit::types::ToolOutput::error("forbidden"));
                }
                let logs = svc.audit_log.get_tap_logs(tap_id, page, limit).await.map_err(map_core)?;
                json_ok(&logs)
            })
        },
    );
}
