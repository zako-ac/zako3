//! WASM mapper tools (mirrors `handlers::mapper`). All require an admin user.
//!
//! The REST create/update endpoints use multipart uploads; here the WASM module
//! is passed as a JSON byte array (`wasm_bytes`) to keep tool inputs JSON-only.

use crate::mcp::auth::require_admin;
use crate::mcp::support::{json_ok, map_core, mk_tool, parse_args, run, text_ok};
use hq_core::Service;
use hq_types::hq::mapper::{
    EvaluateRequestDto, EvaluateSingleRequestDto, MapperInputData, PipelineOrderDto,
};
use mcpkit::server::capability::tools::ToolService;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Deserialize)]
struct MapperRef {
    id: String,
}

#[derive(Deserialize)]
struct CreateMapperArgs {
    id: String,
    name: String,
    #[serde(default)]
    input_data: Vec<MapperInputData>,
    wasm_bytes: Vec<u8>,
}

#[derive(Deserialize)]
struct UpdateMapperArgs {
    id: String,
    name: String,
    #[serde(default)]
    input_data: Vec<MapperInputData>,
    wasm_bytes: Option<Vec<u8>>,
}

#[derive(Deserialize)]
struct EvaluateMapperArgs {
    id: String,
    #[serde(flatten)]
    body: EvaluateSingleRequestDto,
}

pub fn register(tools: &mut ToolService, service: &Arc<Service>) {
    let svc = service.clone();
    tools.register(
        mk_tool(
            "admin_list_mappers",
            "Admin: list registered WASM mappers.",
            json!({"type": "object"}),
        ),
        move |_args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let mappers = svc.mapping.list_mappers().await.map_err(map_core)?;
                json_ok(&mappers)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "admin_get_mapper",
            "Admin: get a mapper by id.",
            json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let MapperRef { id } = parse_args(args)?;
                let mapper = svc.mapping.get_mapper(&id).await.map_err(map_core)?;
                json_ok(&mapper)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "admin_create_mapper",
            "Admin: create a WASM mapper. Provide id, name, optional input_data, and wasm_bytes (array of bytes).",
            json!({"type": "object", "properties": {"id": {"type": "string"}, "name": {"type": "string"}, "input_data": {"type": "array"}, "wasm_bytes": {"type": "array", "items": {"type": "integer"}}}, "required": ["id", "name", "wasm_bytes"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let CreateMapperArgs { id, name, input_data, wasm_bytes } = parse_args(args)?;
                let dto = svc
                    .mapping
                    .create_mapper(id, name, input_data, wasm_bytes)
                    .await
                    .map_err(map_core)?;
                json_ok(&dto)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "admin_update_mapper",
            "Admin: update a WASM mapper. Provide id, name, optional input_data, and optional wasm_bytes.",
            json!({"type": "object", "properties": {"id": {"type": "string"}, "name": {"type": "string"}, "input_data": {"type": "array"}, "wasm_bytes": {"type": "array", "items": {"type": "integer"}}}, "required": ["id", "name"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let UpdateMapperArgs { id, name, input_data, wasm_bytes } = parse_args(args)?;
                let dto = svc
                    .mapping
                    .update_mapper(id, name, input_data, wasm_bytes)
                    .await
                    .map_err(map_core)?;
                json_ok(&dto)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "admin_delete_mapper",
            "Admin: delete a mapper by id.",
            json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
        ),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let MapperRef { id } = parse_args(args)?;
                svc.mapping.delete_mapper(&id).await.map_err(map_core)?;
                text_ok("deleted")
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool(
            "admin_get_mapper_pipeline",
            "Admin: get the current mapper pipeline order.",
            json!({"type": "object"}),
        ),
        move |_args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let pipeline = svc.mapping.get_pipeline().await.map_err(map_core)?;
                json_ok(&pipeline)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool("admin_set_mapper_pipeline", "Admin: set the mapper pipeline order. Body is a PipelineOrderDto.", json!({"type": "object", "properties": {"mapper_ids": {"type": "array"}}, "required": ["mapper_ids"]})),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let payload: PipelineOrderDto = parse_args(args)?;
                svc.mapping.set_pipeline(payload.mapper_ids).await.map_err(map_core)?;
                text_ok("updated")
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool("admin_evaluate_pipeline", "Admin: evaluate text through the pipeline. Body is an EvaluateRequestDto.", json!({"type": "object", "properties": {"text": {"type": "string"}, "mapper_ids": {"type": "array"}}, "required": ["text"]})),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let payload: EvaluateRequestDto = parse_args(args)?;
                let result = svc
                    .mapping
                    .evaluate_pipeline(payload.text, payload.mapper_ids)
                    .await
                    .map_err(map_core)?;
                json_ok(&result)
            })
        },
    );

    let svc = service.clone();
    tools.register(
        mk_tool("admin_evaluate_mapper", "Admin: evaluate text through a single mapper. Provide id and text.", json!({"type": "object", "properties": {"id": {"type": "string"}, "text": {"type": "string"}}, "required": ["id", "text"]})),
        move |args, _ctx| {
            let svc = svc.clone();
            run(async move {
                require_admin()?;
                let EvaluateMapperArgs { id, body } = parse_args(args)?;
                let result = svc.mapping.evaluate_mapper(id, body.text).await.map_err(map_core)?;
                json_ok(&result)
            })
        },
    );
}
