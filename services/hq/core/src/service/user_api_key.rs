use crate::repo::UserApiKeyRepository;
use crate::service::auth::Claims;
use crate::service::validation::validate_api_key_label;
use crate::{AppConfig, CoreError, CoreResult};
use chrono::{DateTime, Duration, Utc};
use hq_types::hq::{
    CreateUserApiKeyDto, UpdateUserApiKeyDto, UserApiKey, UserApiKeyDto, UserApiKeyExpiry,
    UserApiKeyId, UserApiKeyResponseDto, UserId,
};
use jsonwebtoken::{EncodingKey, Header, encode};
use std::sync::Arc;

#[derive(Clone)]
pub struct UserApiKeyService {
    repo: Arc<dyn UserApiKeyRepository>,
    config: Arc<AppConfig>,
}

fn expiry_to_datetime(expiry: UserApiKeyExpiry, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let dur = match expiry {
        UserApiKeyExpiry::OneMonth => Duration::days(30),
        UserApiKeyExpiry::ThreeMonths => Duration::days(90),
        UserApiKeyExpiry::SixMonths => Duration::days(180),
        UserApiKeyExpiry::OneYear => Duration::days(365),
        UserApiKeyExpiry::Never => return None,
    };
    now.checked_add_signed(dur)
}

fn to_dto(key: UserApiKey) -> UserApiKeyDto {
    UserApiKeyDto {
        id: key.id.0,
        label: key.label,
        expires_at: key.expires_at,
        last_used_at: key.last_used_at,
        revoked: key.revoked_at.is_some(),
        created_at: key.created_at,
    }
}

impl UserApiKeyService {
    pub fn new(repo: Arc<dyn UserApiKeyRepository>, config: Arc<AppConfig>) -> Self {
        Self { repo, config }
    }

    pub async fn create_key(
        &self,
        user_id: UserId,
        dto: CreateUserApiKeyDto,
    ) -> CoreResult<UserApiKeyResponseDto> {
        validate_api_key_label(&dto.label)?;

        let now = Utc::now();
        let expires_at = expiry_to_datetime(dto.expiry, now);
        let id = hq_types::hq::next_id().to_string();

        // `Validation::default()` requires `exp`, so for "never" we still sign a
        // far-future expiry; revocation/expiry is enforced via the DB row.
        let exp = expires_at
            .unwrap_or_else(|| now + Duration::days(365 * 100))
            .timestamp() as usize;

        let claims = Claims {
            sub: user_id.0.clone(),
            exp,
            jti: Some(id.clone()),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )?;

        let key = UserApiKey {
            id: UserApiKeyId(id),
            user_id: user_id.clone(),
            label: dto.label,
            expires_at,
            last_used_at: None,
            revoked_at: None,
            created_at: now,
        };

        let created = self.repo.create(&key).await?;
        tracing::info!(user_id = %user_id.0, key_id = %created.id.0, "user api key created");

        Ok(UserApiKeyResponseDto {
            api_key: to_dto(created),
            token,
        })
    }

    pub async fn list_keys(&self, user_id: UserId) -> CoreResult<Vec<UserApiKeyDto>> {
        let keys = self.repo.list_by_user(user_id).await?;
        Ok(keys.into_iter().map(to_dto).collect())
    }

    pub async fn update_key(
        &self,
        user_id: UserId,
        key_id: UserApiKeyId,
        dto: UpdateUserApiKeyDto,
    ) -> CoreResult<UserApiKeyDto> {
        let key = self.find_owned(&user_id, &key_id).await?;

        let Some(label) = dto.label else {
            // Nothing to change; return the current state.
            return Ok(to_dto(key));
        };
        validate_api_key_label(&label)?;

        let updated = self.repo.update_label(key_id, &label).await?;
        Ok(to_dto(updated))
    }

    pub async fn revoke_key(&self, user_id: UserId, key_id: UserApiKeyId) -> CoreResult<()> {
        self.find_owned(&user_id, &key_id).await?;
        self.repo.revoke(key_id.clone()).await?;
        tracing::info!(user_id = %user_id.0, key_id = %key_id.0, "user api key revoked");
        Ok(())
    }

    /// Validate a presented jti from a JWT: it must reference an existing,
    /// non-revoked, non-expired key. Best-effort updates `last_used_at`.
    pub async fn validate_jti(&self, jti: &str) -> CoreResult<()> {
        let key = self
            .repo
            .find_active(UserApiKeyId(jti.to_string()))
            .await?
            .ok_or_else(|| CoreError::Unauthorized("API key revoked or unknown".to_string()))?;

        if let Some(expires_at) = key.expires_at
            && expires_at < Utc::now()
        {
            return Err(CoreError::Unauthorized("API key has expired".to_string()));
        }

        let _ = self.repo.touch_last_used(key.id).await;
        Ok(())
    }

    async fn find_owned(&self, user_id: &UserId, key_id: &UserApiKeyId) -> CoreResult<UserApiKey> {
        let key = self
            .repo
            .find_by_id(key_id.clone())
            .await?
            .ok_or_else(|| CoreError::NotFound("API key not found".to_string()))?;

        if &key.user_id != user_id {
            return Err(CoreError::NotFound("API key not found".to_string()));
        }
        Ok(key)
    }
}
