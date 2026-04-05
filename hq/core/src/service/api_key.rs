use crate::repo::{ApiKeyRepository, TapRepository};
use crate::service::audit_log::AuditLogService;
use crate::{CoreError, CoreResult};
use chrono::Utc;
use hq_types::hq::{
    ApiKey, ApiKeyDto, ApiKeyId, ApiKeyResponseDto, CreateApiKeyDto, TapId, UpdateApiKeyDto,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

fn generate_api_key_secret() -> String {
    format!("zk_{}", Uuid::new_v4().to_string().replace("-", ""))
}

fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Clone)]
pub struct ApiKeyService {
    repo: Arc<dyn ApiKeyRepository>,
    tap_repo: Arc<dyn TapRepository>,
    audit_log: AuditLogService,
}

impl ApiKeyService {
    pub fn new(
        repo: Arc<dyn ApiKeyRepository>,
        tap_repo: Arc<dyn TapRepository>,
        audit_log: AuditLogService,
    ) -> Self {
        Self {
            repo,
            tap_repo,
            audit_log,
        }
    }

    async fn check_tap_ownership(&self, tap_id: Uuid, user_id: Uuid) -> CoreResult<()> {
        let tap = self
            .tap_repo
            .find_by_id(tap_id)
            .await?
            .ok_or_else(|| CoreError::NotFound("Tap not found".to_string()))?;

        if tap.owner_id.0 != user_id {
            return Err(CoreError::Unauthorized(
                "You do not own this tap".to_string(),
            ));
        }

        Ok(())
    }

    pub async fn create_key(
        &self,
        tap_id: Uuid,
        user_id: Uuid,
        dto: CreateApiKeyDto,
    ) -> CoreResult<ApiKeyResponseDto> {
        self.check_tap_ownership(tap_id, user_id).await?;

        let secret = generate_api_key_secret();
        let key_hash = hash_api_key(&secret);

        let key = ApiKey {
            id: ApiKeyId(Uuid::new_v4()),
            tap_id: TapId(tap_id),
            label: dto.label.clone(),
            key_hash,
            expires_at: dto.expires_at,
            last_used_at: None,
            created_at: Utc::now(),
        };

        let created = self.repo.create(&key).await?;

        let _ = self.audit_log.log(
            tap_id,
            Some(user_id),
            "api_key.create".to_string(),
            Some(serde_json::json!({ "key_id": created.id.0.to_string(), "label": created.label, "expires_at": created.expires_at }))
        ).await;

        Ok(ApiKeyResponseDto {
            api_key: ApiKeyDto {
                id: created.id.0.to_string(),
                tap_id: created.tap_id.0.to_string(),
                label: created.label,
                expires_at: created.expires_at,
                last_used_at: created.last_used_at,
                created_at: created.created_at,
            },
            token: secret,
        })
    }

    pub async fn list_keys(&self, tap_id: Uuid, user_id: Uuid) -> CoreResult<Vec<ApiKeyDto>> {
        self.check_tap_ownership(tap_id, user_id).await?;

        let keys = self.repo.list_by_tap(tap_id).await?;
        Ok(keys
            .into_iter()
            .map(|k| ApiKeyDto {
                id: k.id.0.to_string(),
                tap_id: k.tap_id.0.to_string(),
                label: k.label,
                expires_at: k.expires_at,
                last_used_at: k.last_used_at,
                created_at: k.created_at,
            })
            .collect())
    }

    pub async fn update_key(
        &self,
        tap_id: Uuid,
        key_id: Uuid,
        user_id: Uuid,
        dto: UpdateApiKeyDto,
    ) -> CoreResult<ApiKeyDto> {
        self.check_tap_ownership(tap_id, user_id).await?;

        let mut key = self
            .repo
            .find_by_id(key_id)
            .await?
            .ok_or_else(|| CoreError::NotFound("API Key not found".to_string()))?;

        if key.tap_id.0 != tap_id {
            return Err(CoreError::NotFound("API Key not found".to_string()));
        }

        let mut changes = serde_json::Map::new();
        changes.insert(
            "key_id".to_string(),
            serde_json::Value::String(key_id.to_string()),
        );

        if let Some(label) = dto.label {
            changes.insert(
                "label".to_string(),
                serde_json::Value::String(label.clone()),
            );
            key.label = label;
        }

        if let Some(expires_at) = dto.expires_at {
            let expires_at_val =
                serde_json::to_value(expires_at).unwrap_or(serde_json::Value::Null);
            changes.insert("expires_at".to_string(), expires_at_val);
            key.expires_at = expires_at;
        }

        let updated = self.repo.update(&key).await?;

        let _ = self
            .audit_log
            .log(
                tap_id,
                Some(user_id),
                "api_key.update".to_string(),
                Some(serde_json::Value::Object(changes)),
            )
            .await;

        Ok(ApiKeyDto {
            id: updated.id.0.to_string(),
            tap_id: updated.tap_id.0.to_string(),
            label: updated.label,
            expires_at: updated.expires_at,
            last_used_at: updated.last_used_at,
            created_at: updated.created_at,
        })
    }

    pub async fn delete_key(&self, tap_id: Uuid, key_id: Uuid, user_id: Uuid) -> CoreResult<()> {
        self.check_tap_ownership(tap_id, user_id).await?;

        let key = self
            .repo
            .find_by_id(key_id)
            .await?
            .ok_or_else(|| CoreError::NotFound("API Key not found".to_string()))?;

        if key.tap_id.0 != tap_id {
            return Err(CoreError::NotFound("API Key not found".to_string()));
        }

        self.repo.delete(key_id).await?;

        let _ = self
            .audit_log
            .log(
                tap_id,
                Some(user_id),
                "api_key.delete".to_string(),
                Some(serde_json::json!({ "key_id": key_id.to_string(), "label": key.label })),
            )
            .await;

        Ok(())
    }

    pub async fn regenerate_key(
        &self,
        tap_id: Uuid,
        key_id: Uuid,
        user_id: Uuid,
    ) -> CoreResult<ApiKeyResponseDto> {
        self.check_tap_ownership(tap_id, user_id).await?;

        let mut key = self
            .repo
            .find_by_id(key_id)
            .await?
            .ok_or_else(|| CoreError::NotFound("API Key not found".to_string()))?;

        if key.tap_id.0 != tap_id {
            return Err(CoreError::NotFound("API Key not found".to_string()));
        }

        let secret = generate_api_key_secret();
        key.key_hash = hash_api_key(&secret);

        let updated = self.repo.update(&key).await?;

        let _ = self
            .audit_log
            .log(
                tap_id,
                Some(user_id),
                "api_key.regenerate".to_string(),
                Some(serde_json::json!({ "key_id": key_id.to_string() })),
            )
            .await;

        Ok(ApiKeyResponseDto {
            api_key: ApiKeyDto {
                id: updated.id.0.to_string(),
                tap_id: updated.tap_id.0.to_string(),
                label: updated.label,
                expires_at: updated.expires_at,
                last_used_at: updated.last_used_at,
                created_at: updated.created_at,
            },
            token: secret,
        })
    }

    pub async fn authenticate_tap(&self, token: &str) -> CoreResult<Option<hq_types::hq::Tap>> {
        let hash = hash_api_key(token);
        if let Some(mut key) = self.repo.find_by_key_hash(&hash).await? {
            if let Some(expires_at) = key.expires_at {
                if expires_at < Utc::now() {
                    return Err(CoreError::Unauthorized("API Key has expired".to_string()));
                }
            }

            key.last_used_at = Some(Utc::now());
            let _ = self.repo.update(&key).await;
            return self.tap_repo.find_by_id(key.tap_id.0).await;
        }
        Ok(None)
    }
}
