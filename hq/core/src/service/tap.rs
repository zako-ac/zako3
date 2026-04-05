use crate::repo::{TapRepository, UserRepository};
use crate::service::audit_log::AuditLogService;
use crate::service::validation::{validate_tap_description, validate_tap_name};
use crate::{CoreError, CoreResult};
use hq_types::hq::{
    CreateTapDto, PaginatedResponseDto, PaginationMetaDto, Tap, TapDto, TapId, TapStatsDto,
    TapWithAccessDto, TimeSeriesPointDto, UserId, UserSummaryDto,
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

    pub async fn create(&self, owner_id: UserId, dto: CreateTapDto) -> CoreResult<Tap> {
        validate_tap_name(&dto.name)?;
        validate_tap_description(&dto.description)?;

        let mut tap = Tap::new(
            hq_types::hq::next_id().to_string(),
            owner_id.0.clone(),
            dto.name.clone(),
        );
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
                created_tap.id.0.clone(),
                Some(owner_id.0),
                "tap.create".to_string(),
                Some(serde_json::json!({ "name": dto.name, "description": dto.description, "roles": dto.roles, "permission": dto.permission })),
            )
            .await;

        Ok(created_tap)
    }

    pub async fn list_by_user(
        &self,
        user_id: UserId,
    ) -> CoreResult<PaginatedResponseDto<TapWithAccessDto>> {
        let taps = self.tap_repo.list_by_owner(user_id.clone()).await?;
        let user = self
            .user_repo
            .find_by_id(user_id)
            .await?
            .ok_or(CoreError::NotFound("User not found".to_string()))?;

        let mut tap_dtos = Vec::new();
        for tap in taps {
            let tap_dto = self.map_to_tap_dto(tap).await;

            let tap_with_access = TapWithAccessDto {
                tap: tap_dto,
                has_access: true,
                owner: UserSummaryDto {
                    id: user.id.0.clone(),
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

    pub async fn list_all_paginated(
        &self,
        user_id: Option<UserId>,
    ) -> CoreResult<PaginatedResponseDto<TapWithAccessDto>> {
        let taps = self.tap_repo.list_all().await?;

        let mut tap_dtos = Vec::new();
        for tap in taps {
            let owner = self
                .user_repo
                .find_by_id(tap.owner_id.clone())
                .await?
                .ok_or(CoreError::NotFound("Owner not found".to_string()))?;

            let has_access = self.check_access(&tap, user_id.clone()).await;

            let tap_dto = self.map_to_tap_dto(tap).await;

            let tap_with_access = TapWithAccessDto {
                tap: tap_dto,
                has_access,
                owner: UserSummaryDto {
                    id: owner.id.0.clone(),
                    username: owner.username.0.clone(),
                    avatar: owner.avatar_url.clone().unwrap_or_default(),
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
        tap_id: TapId,
        user_id: Option<UserId>,
    ) -> CoreResult<TapWithAccessDto> {
        let tap = self
            .tap_repo
            .find_by_id(tap_id)
            .await?
            .ok_or(CoreError::NotFound("Tap not found".to_string()))?;

        let owner = self
            .user_repo
            .find_by_id(tap.owner_id.clone())
            .await?
            .ok_or(CoreError::NotFound("Owner not found".to_string()))?;

        let has_access = self.check_access(&tap, user_id).await;

        let tap_dto = self.map_to_tap_dto(tap).await;

        Ok(TapWithAccessDto {
            tap: tap_dto,
            has_access,
            owner: UserSummaryDto {
                id: owner.id.0.clone(),
                username: owner.username.0.clone(),
                avatar: owner.avatar_url.clone().unwrap_or_default(),
            },
        })
    }

    pub async fn get_tap_stats(&self, tap_id: TapId, user_id: UserId) -> CoreResult<TapStatsDto> {
        let tap = self
            .tap_repo
            .find_by_id(tap_id.clone())
            .await?
            .ok_or(CoreError::NotFound("Tap not found".to_string()))?;

        if tap.owner_id != user_id {
            return Err(CoreError::Forbidden(
                "You do not have access to this tap's stats".to_string(),
            ));
        }

        // Fetch real data from tap_metric service (Redis for real-time)
        let total_uses = self
            .tap_metrics_state
            .get_metric(tap_id.clone(), TapMetricKey::TotalUses)
            .await
            .unwrap_or(0);
        let active_now = self
            .tap_metrics_state
            .get_metric(tap_id.clone(), TapMetricKey::ActiveNow)
            .await
            .unwrap_or(0);
        let unique_users = self
            .tap_metrics_state
            .get_unique_users_count(tap_id.clone())
            .await
            .unwrap_or(0);
        let cache_hits = self
            .tap_metrics_state
            .get_metric(tap_id.clone(), TapMetricKey::CacheHits)
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
            tap_id: tap.id.0.clone(),
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
        tap_id: TapId,
        user_id: UserId,
        dto: hq_types::hq::UpdateTapDto,
    ) -> CoreResult<Tap> {
        let mut tap = self
            .tap_repo
            .find_by_id(tap_id.clone())
            .await?
            .ok_or(CoreError::NotFound("Tap not found".to_string()))?;

        if tap.owner_id != user_id {
            return Err(CoreError::Forbidden(
                "You do not have permission to update this tap".to_string(),
            ));
        }

        let (updated_tap, changes) = self.apply_updates(&mut tap, dto).await?;
        let result = self.tap_repo.update(&updated_tap).await?;

        let _ = self
            .audit_log
            .log(
                tap_id.0,
                Some(user_id.0),
                "tap.update".to_string(),
                Some(serde_json::Value::Object(changes)),
            )
            .await;

        Ok(result)
    }

    pub async fn admin_update_tap(
        &self,
        tap_id: TapId,
        admin_id: UserId,
        dto: hq_types::hq::UpdateTapDto,
    ) -> CoreResult<Tap> {
        let mut tap = self
            .tap_repo
            .find_by_id(tap_id.clone())
            .await?
            .ok_or(CoreError::NotFound("Tap not found".to_string()))?;

        let (updated_tap, changes) = self.apply_updates(&mut tap, dto).await?;
        let result = self.tap_repo.update(&updated_tap).await?;

        let _ = self
            .audit_log
            .log(
                tap_id.0,
                Some(admin_id.0),
                "tap.admin_update".to_string(),
                Some(serde_json::Value::Object(changes)),
            )
            .await;

        Ok(result)
    }

    pub async fn admin_update_occupation(
        &self,
        tap_id: TapId,
        admin_id: UserId,
        dto: hq_types::hq::UpdateOccupationDto,
    ) -> CoreResult<Tap> {
        let mut tap = self
            .tap_repo
            .find_by_id(tap_id.clone())
            .await?
            .ok_or(CoreError::NotFound("Tap not found".to_string()))?;

        tap.occupation = dto.occupation.clone();
        tap.timestamp.updated_at = chrono::Utc::now();

        let result = self.tap_repo.update(&tap).await?;

        let _ = self
            .audit_log
            .log(
                tap_id.0,
                Some(admin_id.0),
                "tap.admin_update_occupation".to_string(),
                Some(serde_json::json!({ "occupation": dto.occupation })),
            )
            .await;

        Ok(result)
    }

    async fn apply_updates(
        &self,
        tap: &mut Tap,
        dto: hq_types::hq::UpdateTapDto,
    ) -> CoreResult<(Tap, serde_json::Map<String, serde_json::Value>)> {
        let mut changes = serde_json::Map::new();

        if let Some(name) = &dto.name {
            validate_tap_name(name)?;
            changes.insert("name".to_string(), serde_json::Value::String(name.clone()));
            tap.name = hq_types::hq::TapName(name.clone());
        }
        if let Some(description) = &dto.description {
            validate_tap_description(&Some(description.clone()))?;
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

        Ok((tap.clone(), changes))
    }

    pub async fn delete_tap(&self, tap_id: TapId, user_id: UserId) -> CoreResult<()> {
        let tap = self
            .tap_repo
            .find_by_id(tap_id.clone())
            .await?
            .ok_or(CoreError::NotFound("Tap not found".to_string()))?;
        if tap.owner_id != user_id {
            return Err(CoreError::Forbidden(
                "You do not have permission to delete this tap".to_string(),
            ));
        }
        self.tap_repo.delete(tap_id.clone()).await?;

        let _ = self
            .audit_log
            .log(tap_id.0, Some(user_id.0), "tap.delete".to_string(), None)
            .await;

        Ok(())
    }

    pub async fn get_tap_internal(&self, tap_id: TapId) -> CoreResult<Option<Tap>> {
        self.tap_repo.find_by_id(tap_id).await
    }

    pub async fn list_all_taps(&self) -> CoreResult<Vec<Tap>> {
        self.tap_repo.list_all().await
    }

    pub async fn list_taps_by_owner(&self, owner_id: UserId) -> CoreResult<Vec<Tap>> {
        self.tap_repo.list_by_owner(owner_id).await
    }

    pub async fn delete_tap_internal(&self, tap_id: TapId) -> CoreResult<()> {
        self.tap_repo.delete(tap_id).await
    }

    pub async fn get_user_by_discord_id(
        &self,
        discord_id: &str,
    ) -> CoreResult<Option<hq_types::hq::User>> {
        self.user_repo.find_by_discord_id(discord_id).await
    }

    async fn check_access(&self, tap: &Tap, user_id: Option<UserId>) -> bool {
        use hq_types::hq::TapPermission;

        match &tap.permission {
            TapPermission::Public => true,
            TapPermission::OwnerOnly => user_id.map(|id| id == tap.owner_id).unwrap_or(false),
            TapPermission::Whitelisted { user_ids } => {
                if let Some(uid) = user_id {
                    if uid == tap.owner_id {
                        return true;
                    }
                    if let Ok(Some(user)) = self.user_repo.find_by_id(uid).await {
                        return user_ids.contains(&user.discord_user_id.0);
                    }
                }
                false
            }
            TapPermission::Blacklisted { user_ids } => {
                if let Some(uid) = user_id {
                    if uid == tap.owner_id {
                        return true;
                    }
                    if let Ok(Some(user)) = self.user_repo.find_by_id(uid).await {
                        return !user_ids.contains(&user.discord_user_id.0);
                    }
                }
                true
            }
        }
    }

    async fn map_to_tap_dto(&self, tap: Tap) -> TapDto {
        let total_uses = self
            .tap_metrics_state
            .get_metric(tap.id.clone(), TapMetricKey::TotalUses)
            .await
            .unwrap_or(0);
        let cache_hits = self
            .tap_metrics_state
            .get_metric(tap.id.clone(), TapMetricKey::CacheHits)
            .await
            .unwrap_or(0);

        TapDto {
            id: tap.id.0.clone(),
            name: tap.name.0.clone(),
            description: tap.description.clone().unwrap_or_default(),
            owner_id: tap.owner_id.0.clone(),
            occupation: tap.occupation.clone(),
            permission: tap.permission.clone(),
            roles: tap.roles.clone(),
            total_uses,
            cache_hits,
            created_at: tap.timestamp.created_at,
            updated_at: tap.timestamp.updated_at,
        }
    }
}
