//! Tap API-token tools (mirrors `handlers::api_key`). All require the tap owner.

use crate::mcp::auth::require_user;
use crate::mcp::support::{json_ok, map_core, mk_tool, parse_args, run, text_ok};
use hq_core::Service;
use hq_types::hq::{ApiKeyId, CreateApiKeyDto, TapId, UpdateApiKeyDto};
use mcpkit::server::capability::tools::ToolService;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Deserialize)]
struct TapRef {
    tap_id: TapId,
}

#[derive(Deserialize)]
struct KeyRef {
    tap_id: TapId,
    key_id: ApiKeyId,
}

#[derive(Deserialize)]
struct CreateKeyArgs {
    tap_id: TapId,
    #[serde(flatten)]
    body: CreateApiKeyDto,
}

#[derive(Deserialize)]
struct UpdateKeyArgs {
    tap_id: TapId,
    key_id: ApiKeyId,
    #[serde(flatten)]
    body: UpdateApiKeyDto,
}

pub fn register(tools: &mut ToolService, service: &Arc<Service>) {
    let svc = service.clone();
    tools.register(
        mk_tool(
            "create_tap_api_token",
            "Create an API token for a tap. Provide tap_id plus CreateApiKeyDto fields.",
            json!({"type": "object", "properties": {"tap_id": {"type": "string"}}, "required": ["tap_id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let CreateKeyArgs { tap_id, body } = parse_args(args)?;
                let res = svc.api_key.create_key(tap_id, uid, body).await.map_err(map_core)?;
                json_ok(&res)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "list_tap_api_tokens",
            "List API tokens for a tap.",
            json!({"type": "object", "properties": {"tap_id": {"type": "string"}}, "required": ["tap_id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let TapRef { tap_id } = parse_args(args)?;
                let res = svc.api_key.list_keys(tap_id, uid).await.map_err(map_core)?;
                json_ok(&res)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "update_tap_api_token",
            "Update an API token. Provide tap_id, key_id plus UpdateApiKeyDto fields.",
            json!({"type": "object", "properties": {"tap_id": {"type": "string"}, "key_id": {"type": "string"}}, "required": ["tap_id", "key_id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let UpdateKeyArgs { tap_id, key_id, body } = parse_args(args)?;
                let res = svc.api_key.update_key(tap_id, key_id, uid, body).await.map_err(map_core)?;
                json_ok(&res)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "delete_tap_api_token",
            "Delete an API token.",
            json!({"type": "object", "properties": {"tap_id": {"type": "string"}, "key_id": {"type": "string"}}, "required": ["tap_id", "key_id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let KeyRef { tap_id, key_id } = parse_args(args)?;
                svc.api_key.delete_key(tap_id, key_id, uid).await.map_err(map_core)?;
                text_ok("deleted")
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "regenerate_tap_api_token",
            "Regenerate an API token's secret.",
            json!({"type": "object", "properties": {"tap_id": {"type": "string"}, "key_id": {"type": "string"}}, "required": ["tap_id", "key_id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                let uid = require_user()?;
                let KeyRef { tap_id, key_id } = parse_args(args)?;
                let res = svc.api_key.regenerate_key(tap_id, key_id, uid).await.map_err(map_core)?;
                json_ok(&res)
            })
        },
    );
}
