use crate::middleware::auth::AdminUser;
use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::StatusCode,
};
use hq_core::{CoreError, Service};
use hq_types::hq::mapper::{
    EvaluateRequestDto, EvaluateResultDto, EvaluateSingleRequestDto, MapperInputData,
    PipelineOrderDto, WasmMapperDto,
};
use std::sync::Arc;

#[derive(serde::Serialize)]
pub struct ApiErrorResponse {
    pub code: String,
    pub message: String,
}

fn map_error(e: CoreError) -> (StatusCode, Json<ApiErrorResponse>) {
    let (status, code, message) = match e {
        CoreError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg),
        CoreError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, "INVALID_INPUT", msg),
        CoreError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg),
        CoreError::Forbidden(msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg),
        CoreError::Conflict(msg) => (StatusCode::CONFLICT, "CONFLICT", msg),
        other => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", other.to_string()),
    };
    (status, Json(ApiErrorResponse {
        code: code.to_string(),
        message,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/mappers",
    responses(
        (status = 200, description = "List of registered WASM mappers", body = Vec<WasmMapperDto>)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_mappers(
    State(service): State<Arc<Service>>,
    AdminUser(_admin): AdminUser,
) -> Result<Json<Vec<WasmMapperDto>>, (StatusCode, Json<ApiErrorResponse>)> {
    service
        .mapping
        .list_mappers()
        .await
        .map(Json)
        .map_err(map_error)
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/mappers/{id}",
    params(("id" = String, Path, description = "Mapper ID")),
    responses(
        (status = 200, description = "Mapper details", body = WasmMapperDto),
        (status = 404, description = "Mapper not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_mapper(
    State(service): State<Arc<Service>>,
    AdminUser(_admin): AdminUser,
    Path(id): Path<String>,
) -> Result<Json<WasmMapperDto>, (StatusCode, Json<ApiErrorResponse>)> {
    service
        .mapping
        .get_mapper(&id)
        .await
        .map(Json)
        .map_err(map_error)
}

/// Parse multipart fields common to create and update.
async fn parse_multipart(
    mut multipart: Multipart,
) -> Result<(Option<String>, Option<String>, Vec<MapperInputData>, Option<Vec<u8>>), (StatusCode, Json<ApiErrorResponse>)> {
    let mut id: Option<String> = None;
    let mut name: Option<String> = None;
    let mut input_data: Vec<MapperInputData> = vec![];
    let mut wasm_bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| {
            (StatusCode::BAD_REQUEST, Json(ApiErrorResponse {
                code: "MULTIPART_ERROR".to_string(),
                message: e.to_string(),
            }))
        })? {
        match field.name() {
            Some("id") => {
                id = field.text().await.map(Some).map_err(|e| (StatusCode::BAD_REQUEST, Json(ApiErrorResponse {
                    code: "MULTIPART_ERROR".to_string(),
                    message: e.to_string(),
                })))?;
            }
            Some("name") => {
                name = field.text().await.map(Some).map_err(|e| (StatusCode::BAD_REQUEST, Json(ApiErrorResponse {
                    code: "MULTIPART_ERROR".to_string(),
                    message: e.to_string(),
                })))?;
            }
            Some("input_data") => {
                let text = field.text().await.map_err(|e| (StatusCode::BAD_REQUEST, Json(ApiErrorResponse {
                    code: "MULTIPART_ERROR".to_string(),
                    message: e.to_string(),
                })))?;
                input_data = serde_json::from_str(&text)
                    .map_err(|e| (StatusCode::BAD_REQUEST, Json(ApiErrorResponse {
                        code: "INVALID_JSON".to_string(),
                        message: format!("Invalid input_data JSON: {}", e),
                    })))?;
            }
            Some("file") => {
                let bytes = field.bytes().await.map_err(|e| (StatusCode::BAD_REQUEST, Json(ApiErrorResponse {
                    code: "MULTIPART_ERROR".to_string(),
                    message: e.to_string(),
                })))?;
                wasm_bytes = Some(bytes.to_vec());
            }
            _ => {}
        }
    }

    Ok((id, name, input_data, wasm_bytes))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/mappers",
    responses(
        (status = 201, description = "Mapper created", body = WasmMapperDto),
        (status = 400, description = "Missing required fields")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_mapper(
    State(service): State<Arc<Service>>,
    AdminUser(_admin): AdminUser,
    multipart: Multipart,
) -> Result<(StatusCode, Json<WasmMapperDto>), (StatusCode, Json<ApiErrorResponse>)> {
    let (id, name, input_data, wasm_bytes) = parse_multipart(multipart).await?;

    let id = id.ok_or_else(|| (StatusCode::BAD_REQUEST, Json(ApiErrorResponse {
        code: "MISSING_FIELD".to_string(),
        message: "Missing 'id' field".to_string(),
    })))?;
    let name = name.ok_or_else(|| (StatusCode::BAD_REQUEST, Json(ApiErrorResponse {
        code: "MISSING_FIELD".to_string(),
        message: "Missing 'name' field".to_string(),
    })))?;
    let wasm_bytes = wasm_bytes.ok_or_else(|| (StatusCode::BAD_REQUEST, Json(ApiErrorResponse {
        code: "MISSING_FIELD".to_string(),
        message: "Missing 'file' field".to_string(),
    })))?;

    service
        .mapping
        .create_mapper(id, name, input_data, wasm_bytes)
        .await
        .map(|dto| (StatusCode::CREATED, Json(dto)))
        .map_err(map_error)
}

#[utoipa::path(
    put,
    path = "/api/v1/admin/mappers/{id}",
    params(("id" = String, Path, description = "Mapper ID")),
    responses(
        (status = 200, description = "Mapper updated", body = WasmMapperDto),
        (status = 404, description = "Mapper not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_mapper(
    State(service): State<Arc<Service>>,
    AdminUser(_admin): AdminUser,
    Path(id): Path<String>,
    multipart: Multipart,
) -> Result<Json<WasmMapperDto>, (StatusCode, Json<ApiErrorResponse>)> {
    let (_, name, input_data, wasm_bytes) = parse_multipart(multipart).await?;

    let name = name.ok_or_else(|| (StatusCode::BAD_REQUEST, Json(ApiErrorResponse {
        code: "MISSING_FIELD".to_string(),
        message: "Missing 'name' field".to_string(),
    })))?;

    service
        .mapping
        .update_mapper(id, name, input_data, wasm_bytes)
        .await
        .map(Json)
        .map_err(map_error)
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/mappers/{id}",
    params(("id" = String, Path, description = "Mapper ID")),
    responses(
        (status = 204, description = "Mapper deleted"),
        (status = 404, description = "Mapper not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_mapper(
    State(service): State<Arc<Service>>,
    AdminUser(_admin): AdminUser,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ApiErrorResponse>)> {
    service
        .mapping
        .delete_mapper(&id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(map_error)
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/mappers/pipeline",
    responses(
        (status = 200, description = "Current pipeline order", body = PipelineOrderDto)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_pipeline(
    State(service): State<Arc<Service>>,
    AdminUser(_admin): AdminUser,
) -> Result<Json<PipelineOrderDto>, (StatusCode, Json<ApiErrorResponse>)> {
    service
        .mapping
        .get_pipeline()
        .await
        .map(Json)
        .map_err(map_error)
}

#[utoipa::path(
    put,
    path = "/api/v1/admin/mappers/pipeline",
    request_body = PipelineOrderDto,
    responses(
        (status = 204, description = "Pipeline order updated")
    ),
    security(("bearer_auth" = []))
)]
pub async fn set_pipeline(
    State(service): State<Arc<Service>>,
    AdminUser(_admin): AdminUser,
    Json(payload): Json<PipelineOrderDto>,
) -> Result<StatusCode, (StatusCode, Json<ApiErrorResponse>)> {
    service
        .mapping
        .set_pipeline(payload.mapper_ids)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(map_error)
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/mappers/evaluate",
    request_body = EvaluateRequestDto,
    responses(
        (status = 200, description = "Step-by-step evaluation result", body = EvaluateResultDto)
    ),
    security(("bearer_auth" = []))
)]
pub async fn evaluate_pipeline(
    State(service): State<Arc<Service>>,
    AdminUser(_admin): AdminUser,
    Json(payload): Json<EvaluateRequestDto>,
) -> Result<Json<EvaluateResultDto>, (StatusCode, Json<ApiErrorResponse>)> {
    service
        .mapping
        .evaluate_pipeline(payload.text, payload.mapper_ids)
        .await
        .map(Json)
        .map_err(map_error)
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/mappers/{id}/evaluate",
    params(("id" = String, Path, description = "Mapper ID")),
    request_body = EvaluateSingleRequestDto,
    responses(
        (status = 200, description = "Single-mapper evaluation result", body = EvaluateResultDto),
        (status = 404, description = "Mapper not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn evaluate_mapper(
    State(service): State<Arc<Service>>,
    AdminUser(_admin): AdminUser,
    Path(id): Path<String>,
    Json(payload): Json<EvaluateSingleRequestDto>,
) -> Result<Json<EvaluateResultDto>, (StatusCode, Json<ApiErrorResponse>)> {
    service
        .mapping
        .evaluate_mapper(id, payload.text)
        .await
        .map(Json)
        .map_err(map_error)
}
