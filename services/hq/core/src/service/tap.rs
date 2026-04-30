use crate::repo::{TapRepository, UserRepository};
use crate::service::audit_log::AuditLogService;
use crate::service::validation::{validate_tap_description, validate_tap_name};
use crate::{CoreError, CoreResult};
use chrono::Utc;
use hq_types::hq::{
    CreateTapDto, PaginatedResponseDto, PaginationMetaDto, Tap, TapDto, TapId, TapName, TapRole,
    TapStatsDto, TapWithAccessDto, TimeSeriesPointDto, UserId, UserSummaryDto,
};
use serde::Deserialize;
use std::sync::Arc;
use zako3_metrics::TapMetricsService;
use zako3_states::TapHubStateService;

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum TapSortField {
    MostUsed,
    RecentlyCreated,
    Alphabetical,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Clone)]
pub struct TapService {
    tap_repo: Arc<dyn TapRepository>,
    user_repo: Arc<dyn UserRepository>,
    audit_log: AuditLogService,
    tap_metrics: TapMetricsService,
    tap_hub_state: TapHubStateService,
}

impl TapService {
    pub fn new(
        tap_repo: Arc<dyn TapRepository>,
        user_repo: Arc<dyn UserRepository>,
        audit_log: AuditLogService,
        tap_metrics: TapMetricsService,
        tap_hub_state: TapHubStateService,
    ) -> Self {
        Self {
            tap_repo,
            user_repo,
            audit_log,
            tap_metrics,
            tap_hub_state,
        }
    }

    #[tracing::instrument(skip(self, dto), fields(user_id = %owner_id.0), err)]
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
        if let Some(base_volume) = dto.base_volume {
            if !(0.0..=2.0).contains(&base_volume) {
                return Err(CoreError::InvalidInput(
                    "base_volume must be between 0.0 and 2.0".to_string(),
                ));
            }
            tap.base_volume = base_volume;
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
        sort_field: Option<TapSortField>,
        sort_direction: Option<SortDirection>,
        search: Option<String>,
        roles: Option<String>,
        accessible: Option<bool>,
        page: Option<i64>,
        per_page: Option<i64>,
    ) -> CoreResult<PaginatedResponseDto<TapWithAccessDto>> {
        let mut taps = self.tap_repo.list_all().await?;

        // 1. Filter by search (case-insensitive name match) before expensive enrichment
        if let Some(ref q) = search {
            let q_lower = q.to_lowercase();
            taps.retain(|t| t.name.0.to_lowercase().contains(&q_lower));
        }

        // 2. Filter by roles before expensive enrichment
        if let Some(ref roles_str) = roles {
            let requested: Vec<&str> = roles_str.split(',').map(str::trim).collect();
            taps.retain(|t| {
                t.roles.iter().any(|r| {
                    let s = match r {
                        TapRole::Music => "music",
                        TapRole::TTS => "tts",
                    };
                    requested.contains(&s)
                })
            });
        }

        // 3. Enrich surviving taps
        let mut tap_dtos = Vec::new();
        for tap in taps {
            let owner = self
                .user_repo
                .find_by_id(tap.owner_id.clone())
                .await?
                .ok_or(CoreError::NotFound("Owner not found".to_string()))?;

            let has_access = self.check_access(&tap, user_id.clone()).await;
            let tap_dto = self.map_to_tap_dto(tap).await;

            tap_dtos.push(TapWithAccessDto {
                tap: tap_dto,
                has_access,
                owner: UserSummaryDto {
                    id: owner.id.0.clone(),
                    username: owner.username.0.clone(),
                    avatar: owner.avatar_url.clone().unwrap_or_default(),
                },
            });
        }

        // 4. Filter by accessible
        if accessible == Some(true) {
            tap_dtos.retain(|t| t.has_access);
        }

        // 5. Sort
        let desc = sort_direction.as_ref() != Some(&SortDirection::Asc);
        match sort_field.unwrap_or(TapSortField::MostUsed) {
            TapSortField::MostUsed => {
                tap_dtos.sort_by(|a, b| b.tap.total_uses.cmp(&a.tap.total_uses));
            }
            TapSortField::RecentlyCreated => {
                tap_dtos.sort_by(|a, b| b.tap.created_at.cmp(&a.tap.created_at));
            }
            TapSortField::Alphabetical => {
                tap_dtos.sort_by(|a, b| a.tap.name.cmp(&b.tap.name));
            }
        }
        if !desc {
            tap_dtos.reverse();
        }

        // 6. Paginate
        let total = tap_dtos.len() as u64;
        let per_page = per_page.unwrap_or(20).max(1) as u64;
        let page = page.unwrap_or(1).max(1) as u64;
        let total_pages = (total + per_page - 1) / per_page; // ceil division
        let offset = ((page - 1) * per_page) as usize;
        let data = tap_dtos
            .into_iter()
            .skip(offset)
            .take(per_page as usize)
            .collect();

        Ok(PaginatedResponseDto {
            data,
            meta: PaginationMetaDto {
                total,
                page,
                per_page,
                total_pages,
            },
        })
    }

    #[tracing::instrument(skip(self), err)]
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
            .tap_metrics
            .get_latest_total_uses(&tap_id)
            .await
            .unwrap_or(0);
        let active_now = self
            .tap_hub_state
            .get_online_count(&tap_id)
            .await
            .unwrap_or(0) as u64;
        let unique_users = self
            .tap_metrics
            .get_unique_users_count(tap_id.clone())
            .await
            .unwrap_or(0);
        let cache_hits = self
            .tap_metrics
            .get_latest_cache_hits(&tap_id)
            .await
            .unwrap_or(0);

        let now = Utc::now();
        let since = now - chrono::Duration::hours(24);
        let rows = self
            .tap_metrics
            .get_time_series(&tap_id, since)
            .await
            .unwrap_or_default();

        let use_rate_history: Vec<TimeSeriesPointDto> = rows
            .windows(2)
            .map(|w| {
                let prev = w[0].total_uses;
                let curr = w[1].total_uses;
                let ts = w[1].time;
                TimeSeriesPointDto {
                    timestamp: ts.to_rfc3339(),
                    value: (curr - prev).max(0) as f64,
                }
            })
            .collect();

        let cache_hit_rate_history: Vec<TimeSeriesPointDto> = rows
            .iter()
            .map(|row| {
                let total = row.total_uses;
                let hits = row.cache_hits;
                TimeSeriesPointDto {
                    timestamp: row.time.to_rfc3339(),
                    value: if total > 0 {
                        hits as f64 / total as f64 * 100.0
                    } else {
                        0.0
                    },
                }
            })
            .collect();

        let accumulated_uptime = self
            .tap_metrics
            .get_uptime_secs(tap_id.clone())
            .await
            .unwrap_or(0);
        let online_states = self
            .tap_hub_state
            .get_tap_states(&tap_id)
            .await
            .unwrap_or_default();
        let current_session_secs: u64 = online_states
            .iter()
            .map(|s| (chrono::Utc::now() - s.connected_at).num_seconds().max(0) as u64)
            .sum();
        let total_uptime_secs = accumulated_uptime + current_session_secs;
        let tap_age_secs = (chrono::Utc::now() - tap.timestamp.created_at)
            .num_seconds()
            .max(1) as u64;
        let uptime_percent = (total_uptime_secs as f64 / tap_age_secs as f64 * 100.0).min(100.0);

        Ok(TapStatsDto {
            tap_id: tap.id.0.clone(),
            currently_active: active_now,
            total_uses,
            cache_hits,
            unique_users,
            uptime_percent,
            use_rate_history,
            cache_hit_rate_history,
        })
    }

    #[tracing::instrument(skip(self, dto), fields(tap_id = %tap_id.0, user_id = %user_id.0), err)]
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
        if let Some(base_volume) = dto.base_volume {
            if !(0.0..=2.0).contains(&base_volume) {
                return Err(CoreError::InvalidInput(
                    "base_volume must be between 0.0 and 2.0".to_string(),
                ));
            }
            changes.insert("base_volume".to_string(), serde_json::json!(base_volume));
            tap.base_volume = base_volume;
        }
        tap.timestamp.updated_at = chrono::Utc::now();

        Ok((tap.clone(), changes))
    }

    #[tracing::instrument(skip(self), fields(tap_id = %tap_id.0, user_id = %user_id.0), err)]
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

    #[tracing::instrument(skip(self), fields(tap_id = %tap_id.0), err)]
    pub async fn get_tap(&self, tap_id: TapId) -> CoreResult<Option<Tap>> {
        self.tap_repo.find_by_id(tap_id).await
    }

    pub async fn get_tap_by_name(&self, name: &TapName) -> CoreResult<Option<Tap>> {
        self.tap_repo.find_by_name(name).await
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

    pub async fn check_access(&self, tap: &Tap, user_id: Option<UserId>) -> bool {
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
            .tap_metrics
            .get_latest_total_uses(&tap.id)
            .await
            .unwrap_or(0);
        let cache_hits = self
            .tap_metrics
            .get_latest_cache_hits(&tap.id)
            .await
            .unwrap_or(0);
        let unique_users = self
            .tap_metrics
            .get_unique_users_count(tap.id.clone())
            .await
            .unwrap_or(0);
        let accumulated_uptime = self
            .tap_metrics
            .get_uptime_secs(tap.id.clone())
            .await
            .unwrap_or(0);
        let online_states = self
            .tap_hub_state
            .get_tap_states(&tap.id)
            .await
            .unwrap_or_default();
        let active_now = online_states.len() as u64;
        let current_session_secs: u64 = online_states
            .iter()
            .map(|s| (chrono::Utc::now() - s.connected_at).num_seconds().max(0) as u64)
            .sum();
        let total_uptime_secs = accumulated_uptime + current_session_secs;
        let tap_age_secs = (chrono::Utc::now() - tap.timestamp.created_at)
            .num_seconds()
            .max(1) as u64;
        let uptime_percent = (total_uptime_secs as f64 / tap_age_secs as f64 * 100.0).min(100.0);

        TapDto {
            id: tap.id.0.clone(),
            name: tap.name.0.clone(),
            description: tap.description.clone().unwrap_or_default(),
            owner_id: tap.owner_id.0.clone(),
            occupation: tap.occupation.clone(),
            permission: tap.permission.clone(),
            roles: tap.roles.clone(),
            base_volume: tap.base_volume,
            total_uses,
            cache_hits,
            created_at: tap.timestamp.created_at,
            updated_at: tap.timestamp.updated_at,
            stats: TapStatsDto {
                tap_id: tap.id.0.clone(),
                currently_active: active_now,
                total_uses,
                cache_hits,
                unique_users,
                uptime_percent,
                use_rate_history: vec![],
                cache_hit_rate_history: vec![],
            },
        }
    }
}
