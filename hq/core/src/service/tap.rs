use crate::repo::{TapRepository, UserRepository};
use crate::service::audit_log::AuditLogService;
use crate::{CoreError, CoreResult};
use hq_types::hq::{
    CreateTapDto, PaginatedResponseDto, PaginationMetaDto, Tap, TapDto, TapStatsDto,
    TapWithAccessDto, TimeSeriesPointDto, UserSummaryDto,
};
use std::sync::Arc;
use zako3_states::{TapMetricKey, TapMetricsStateService};

#[derive(Clone)]
pub struct TapService {
    tap_repo: Arc<dyn TapRepository>,
    user_repo: Arc<dyn UserRepository>,
    audit_log: AuditLogService,
    tap_metrics_state: TapMetricsStateService,
}

impl TapService {
    pub fn new(
        tap_repo: Arc<dyn TapRepository>,
        user_repo: Arc<dyn UserRepository>,
        audit_log: AuditLogService,
        tap_metrics_state: TapMetricsStateService,
    ) -> Self {
        Self {
            tap_repo,
            user_repo,
            audit_log,
            tap_metrics_state,
        }
    }

    pub async fn create(&self, owner_id: u64, dto: CreateTapDto) -> CoreResult<Tap> {
        let mut tap = Tap::new(hq_types::hq::next_id(), owner_id, dto.name.clone());
        tap.description = dto.description.clone();
        if let Some(permission) = dto.permission.clone() {
            tap.permission = permission;
        }
        if let Some(roles) = dto.roles.clone() {
            tap.roles = roles;
        }

        let created_tap = self.tap_repo.create(&tap).await?;

        let _ = self
            .audit_log
            .log(
                created_tap.id.0,
                Some(owner_id),
                "tap.create".to_string(),
                Some(serde_json::json!({ "name": dto.name, "description": dto.description, "roles": dto.roles, "permission": dto.permission })),
            )
            .await;

        Ok(created_tap)
    }

    pub async fn list_by_user(
        &self,
        user_id: u64,
    ) -> CoreResult<PaginatedResponseDto<TapWithAccessDto>> {
        let taps = self.tap_repo.list_by_owner(user_id).await?;
        let user = self
            .user_repo
            .find_by_id(user_id)
            .await?
            .ok_or(CoreError::NotFound("User not found".to_string()))?;

        let mut tap_dtos = Vec::new();
        for tap in taps {
            let total_uses = self
                .tap_metrics_state
                .get_metric(tap.id, TapMetricKey::TotalUses)
                .await
                .unwrap_or(0);
            let cache_hits = self
                .tap_metrics_state
                .get_metric(tap.id, TapMetricKey::CacheHits)
                .await
                .unwrap_or(0);

            let tap_dto = TapDto {
                id: tap.id.0.to_string(),
                name: tap.name.0.clone(),
                description: tap.description.clone().unwrap_or_default(),
                owner_id: tap.owner_id.0.to_string(),
                occupation: tap.occupation.clone(),
                permission: tap.permission.clone(),
                roles: tap.roles.clone(),
                total_uses,
                cache_hits,
                created_at: tap.timestamp.created_at,
                updated_at: tap.timestamp.updated_at,
            };

            let tap_with_access = TapWithAccessDto {
                tap: tap_dto,
                has_access: true,
                owner: UserSummaryDto {
                    id: user.id.0.to_string(),
                    username: user.username.0.clone(),
                    avatar: user.avatar_url.clone().unwrap_or_default(),
                },
            };

            tap_dtos.push(tap_with_access);
        }

        let total = tap_dtos.len() as u64;
        Ok(PaginatedResponseDto {
            data: tap_dtos,
            meta: PaginationMetaDto {
                total,
                page: 1,
                per_page: 50,
                total_pages: 1,
            },
        })
    }

    pub async fn get_tap_with_access(
        &self,
        tap_id: u64,
        user_id: u64,
    ) -> CoreResult<TapWithAccessDto> {
        let tap = self
            .tap_repo
            .find_by_id(tap_id)
            .await?
            .ok_or(CoreError::NotFound("Tap not found".to_string()))?;

        let owner = self
            .user_repo
            .find_by_id(tap.owner_id.0)
            .await?
            .ok_or(CoreError::NotFound("Owner not found".to_string()))?;

        let has_access = tap.owner_id.0 == user_id; // Simple permission logic for now

        let total_uses = self
            .tap_metrics_state
            .get_metric(tap.id, TapMetricKey::TotalUses)
            .await
            .unwrap_or(0);
        let cache_hits = self
            .tap_metrics_state
            .get_metric(tap.id, TapMetricKey::CacheHits)
            .await
            .unwrap_or(0);

        let tap_dto = TapDto {
            id: tap.id.0.to_string(),
            name: tap.name.0.clone(),
            description: tap.description.clone().unwrap_or_default(),
            owner_id: tap.owner_id.0.to_string(),
            occupation: tap.occupation.clone(),
            permission: tap.permission.clone(),
            roles: tap.roles.clone(),
            total_uses,
            cache_hits,
            created_at: tap.timestamp.created_at,
            updated_at: tap.timestamp.updated_at,
        };

        Ok(TapWithAccessDto {
            tap: tap_dto,
            has_access,
            owner: UserSummaryDto {
                id: owner.id.0.to_string(),
                username: owner.username.0.clone(),
                avatar: owner.avatar_url.clone().unwrap_or_default(),
            },
        })
    }

    pub async fn get_tap_stats(&self, tap_id: u64, user_id: u64) -> CoreResult<TapStatsDto> {
        let tap = self
            .tap_repo
            .find_by_id(tap_id)
            .await?
            .ok_or(CoreError::NotFound("Tap not found".to_string()))?;

        if tap.owner_id.0 != user_id {
            return Err(CoreError::Forbidden(
                "You do not have access to this tap's stats".to_string(),
            ));
        }

        // Fetch real data from tap_metric service (Redis for real-time)
        let tap_id_typed = hq_types::hq::TapId(tap_id);
        let total_uses = self
            .tap_metrics_state
            .get_metric(tap_id_typed, TapMetricKey::TotalUses)
            .await
            .unwrap_or(0);
        let active_now = self
            .tap_metrics_state
            .get_metric(tap_id_typed, TapMetricKey::ActiveNow)
            .await
            .unwrap_or(0);
        let unique_users = self
            .tap_metrics_state
            .get_unique_users_count(tap_id_typed)
            .await
            .unwrap_or(0);
        let cache_hits = self
            .tap_metrics_state
            .get_metric(tap_id_typed, TapMetricKey::CacheHits)
            .await
            .unwrap_or(0);

        // Return mostly mock data for history since we only track basic metrics for MVP
        let now = chrono::Utc::now();
        let mut use_rate_history = Vec::new();
        let mut cache_hit_rate_history = Vec::new();

        for i in 0..12 {
            let t = now - chrono::Duration::hours(11 - i);
            use_rate_history.push(TimeSeriesPointDto {
                timestamp: t.to_rfc3339(),
                value: 42.0,
            });
            cache_hit_rate_history.push(TimeSeriesPointDto {
                timestamp: t.to_rfc3339(),
                value: 85.0,
            });
        }

        Ok(TapStatsDto {
            tap_id: tap.id.0.to_string(),
            currently_active: active_now,
            total_uses,
            cache_hits,
            unique_users,
            use_rate_history,
            cache_hit_rate_history,
        })
    }

    pub async fn update_tap(
        &self,
        tap_id: u64,
        user_id: u64,
        dto: hq_types::hq::UpdateTapDto,
    ) -> CoreResult<Tap> {
        let mut tap = self
            .tap_repo
            .find_by_id(tap_id)
            .await?
            .ok_or(CoreError::NotFound("Tap not found".to_string()))?;

        if tap.owner_id.0 != user_id {
            return Err(CoreError::Forbidden(
                "You do not have permission to update this tap".to_string(),
            ));
        }

        let mut changes = serde_json::Map::new();

        if let Some(name) = &dto.name {
            changes.insert("name".to_string(), serde_json::Value::String(name.clone()));
            tap.name = hq_types::hq::TapName(name.clone());
        }
        if let Some(description) = &dto.description {
            changes.insert(
                "description".to_string(),
                serde_json::Value::String(description.clone()),
            );
            tap.description = Some(description.clone());
        }
        if let Some(permission) = &dto.permission {
            changes.insert(
                "permission".to_string(),
                serde_json::to_value(permission).unwrap_or(serde_json::Value::Null),
            );
            tap.permission = permission.clone();
        }
        if let Some(roles) = &dto.roles {
            changes.insert(
                "roles".to_string(),
                serde_json::to_value(roles).unwrap_or(serde_json::Value::Null),
            );
            tap.roles = roles.clone();
        }
        tap.timestamp.updated_at = chrono::Utc::now();

        let updated_tap = self.tap_repo.update(&tap).await?;

        let _ = self
            .audit_log
            .log(
                tap_id,
                Some(user_id),
                "tap.update".to_string(),
                Some(serde_json::Value::Object(changes)),
            )
            .await;

        Ok(updated_tap)
    }

    pub async fn delete_tap(&self, tap_id: u64, user_id: u64) -> CoreResult<()> {
        let tap = self
            .tap_repo
            .find_by_id(tap_id)
            .await?
            .ok_or(CoreError::NotFound("Tap not found".to_string()))?;
        if tap.owner_id.0 != user_id {
            return Err(CoreError::Forbidden(
                "You do not have permission to delete this tap".to_string(),
            ));
        }
        self.tap_repo.delete(tap_id).await?;

        let _ = self
            .audit_log
            .log(tap_id, Some(user_id), "tap.delete".to_string(), None)
            .await;

        Ok(())
    }

    pub async fn get_tap_internal(&self, tap_id: u64) -> CoreResult<Option<Tap>> {
        self.tap_repo.find_by_id(tap_id).await
    }

    pub async fn list_all_taps(&self) -> CoreResult<Vec<Tap>> {
        self.tap_repo.list_all().await
    }

    pub async fn list_taps_by_owner(&self, owner_id: u64) -> CoreResult<Vec<Tap>> {
        self.tap_repo.list_by_owner(owner_id).await
    }

    pub async fn delete_tap_internal(&self, tap_id: u64) -> CoreResult<()> {
        self.tap_repo.delete(tap_id).await
    }

    pub async fn get_user_by_discord_id(
        &self,
        discord_id: &str,
    ) -> CoreResult<Option<hq_types::hq::User>> {
        self.user_repo.find_by_discord_id(discord_id).await
    }
}
