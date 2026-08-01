use crate::repo::{TapRepository, UserRepository};
use crate::service::audit_log::AuditLogService;
use crate::service::validation::{validate_tap_description, validate_tap_name};
use crate::{CoreError, CoreResult};
use chrono::Utc;
use hq_types::hq::{
    CreateTapDto, PaginatedResponseDto, PaginationMetaDto, Tap, TapDto, TapId, TapName, TapRole,
    TapStatsDto, TapWithAccessDto, TimeSeriesPointDto, User, UserId, UserSummaryDto,
};
use futures_util::stream::{self, StreamExt};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use zako3_metrics::{TapMetricsRow, TapMetricsService};
use zako3_states::{TapHubStateService, TapNamesCacheService};

/// Max taps enriched concurrently. Owner + latest-metrics lookups are batched into
/// one Postgres query each per listing (see `batch_owner_and_rows`), so on the happy
/// path a listing holds at most one sqlx connection regardless of tap count — this
/// bound only caps the remaining per-tap Redis fan-out in `map_to_tap_dto`, not the
/// sqlx pool. Note: taps whose metrics row is missing from the batch map still fall
/// back to a per-tap `get_latest_row` query inside `map_to_tap_dto`, so the
/// single-connection guarantee only holds when every tap already has a metrics row.
const ENRICH_CONCURRENCY: usize = 4;

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
    tap_names_cache: TapNamesCacheService,
}

impl TapService {
    pub fn new(
        tap_repo: Arc<dyn TapRepository>,
        user_repo: Arc<dyn UserRepository>,
        audit_log: AuditLogService,
        tap_metrics: TapMetricsService,
        tap_hub_state: TapHubStateService,
        tap_names_cache: TapNamesCacheService,
    ) -> Self {
        Self {
            tap_repo,
            user_repo,
            audit_log,
            tap_metrics,
            tap_hub_state,
            tap_names_cache,
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

        // All taps share this single owner, so only the latest-metrics rows need batching.
        // Fetch them in one query (mirrors `batch_owner_and_rows`), then enrich concurrently
        // via `enrich_tap` — removing the previous per-tap `map_to_tap_dto(_, None)` N+1.
        let tap_ids: Vec<TapId> = taps.iter().map(|t| t.id.clone()).collect();
        // Propagate batch failures instead of silently falling back to an empty map:
        // an empty map would make every tap take the per-tap `get_latest_row` fallback
        // in `map_to_tap_dto`, reinstating the N+1 this batching was meant to remove.
        let rows = self.tap_metrics.get_latest_rows(&tap_ids).await?;

        let user = &user;
        let tap_dtos: Vec<TapWithAccessDto> = stream::iter(taps)
            .map(|tap| {
                let row = rows.get(&tap.id).cloned();
                async move { self.enrich_tap(tap, true, user, row).await }
            })
            // `.buffered` preserves the input order of `taps`; `buffer_unordered` yields
            // in completion order, which would shuffle the listing for API/bot consumers.
            .buffered(ENRICH_CONCURRENCY)
            .collect()
            .await;

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

    /// Enrich a raw tap into a `TapWithAccessDto`. Both the access decision and the
    /// owner + latest metrics row are resolved by the caller (batched, query-free once
    /// per listing) and passed in; the only remaining work is the Redis-backed dto map.
    async fn enrich_tap(
        &self,
        tap: Tap,
        has_access: bool,
        owner: &User,
        row: Option<TapMetricsRow>,
    ) -> TapWithAccessDto {
        let tap_dto = self.map_to_tap_dto(tap, row).await;
        TapWithAccessDto {
            tap: tap_dto,
            has_access,
            owner: UserSummaryDto {
                id: owner.id.0.clone(),
                username: owner.username.0.clone(),
                avatar: owner.avatar_url.clone().unwrap_or_default(),
            },
        }
    }

    /// Resolve the requester's discord id with a single query so per-tap access checks
    /// (`check_access_resolved`) need no further database round-trips.
    async fn resolve_requester_discord_id(&self, user_id: &Option<UserId>) -> Option<String> {
        match user_id {
            Some(uid) => self
                .user_repo
                .find_by_id(uid.clone())
                .await
                .ok()
                .flatten()
                .map(|u| u.discord_user_id.0),
            None => None,
        }
    }

    /// Batch-fetch, in one Postgres query each, the owner and latest-metrics-row maps for
    /// a set of taps — so the per-tap enrichment fan-out needs no further Postgres
    /// round-trips (removes the previous per-tap owner/metrics N+1). Missing owners/rows
    /// are simply absent from the returned maps.
    async fn batch_owner_and_rows(
        &self,
        taps: &[Tap],
    ) -> CoreResult<(HashMap<UserId, User>, HashMap<TapId, TapMetricsRow>)> {
        let owner_ids: Vec<UserId> = taps.iter().map(|t| t.owner_id.clone()).collect();
        let tap_ids: Vec<TapId> = taps.iter().map(|t| t.id.clone()).collect();
        let (owners, rows) = tokio::join!(
            self.user_repo.find_by_ids(owner_ids),
            self.tap_metrics.get_latest_rows(&tap_ids),
        );
        let owners: HashMap<UserId, User> =
            owners?.into_iter().map(|u| (u.id.clone(), u)).collect();
        let rows = rows.unwrap_or_default();
        Ok((owners, rows))
    }

    /// Returns every tap the user can access, unpaginated, sorted most-used first.
    /// Used by the bot where the full list is required (autocomplete, validation,
    /// listing) — unlike `list_all_paginated`, which caps results for API callers.
    pub async fn list_all_accessible(
        &self,
        user_id: Option<UserId>,
    ) -> CoreResult<Vec<TapWithAccessDto>> {
        let mut taps = self.tap_repo.list_all().await?;

        // Resolve the requester once, then drop inaccessible taps *before* the
        // expensive per-tap enrichment — we only enrich what the user can see.
        let requester_discord_id = self.resolve_requester_discord_id(&user_id).await;
        taps.retain(|t| Self::check_access_resolved(t, &user_id, requester_discord_id.as_deref()));

        // Batch the owner + metrics lookups once, then fan out the Redis-backed dto map.
        let (owners, rows) = self.batch_owner_and_rows(&taps).await?;
        let mut out: Vec<TapWithAccessDto> = stream::iter(taps)
            .map(|tap| {
                let owner = owners.get(&tap.owner_id).cloned();
                let row = rows.get(&tap.id).cloned();
                async move {
                    let owner = match owner {
                        Some(o) => o,
                        None => {
                            tracing::warn!(tap_id = %tap.id, "skipping tap with missing owner");
                            return None;
                        }
                    };
                    Some(self.enrich_tap(tap, true, &owner, row).await)
                }
            })
            .buffer_unordered(ENRICH_CONCURRENCY)
            .filter_map(|x| async move { x })
            .collect()
            .await;

        out.sort_by(|a, b| b.tap.total_uses.cmp(&a.tap.total_uses));
        Ok(out)
    }

    /// Names of every tap the user can access, sorted alphabetically. Used by bot
    /// autocomplete, which needs only names — so this skips the per-tap owner and
    /// metrics enrichment that `list_all_accessible` performs (2 queries total,
    /// independent of tap count).
    pub async fn list_accessible_names(
        &self,
        user_id: Option<UserId>,
    ) -> CoreResult<Vec<String>> {
        if let Some(cached) = self.tap_names_cache.get_cached(&user_id).await {
            return Ok(cached);
        }
        let taps = self.tap_repo.list_all().await?;
        let requester_discord_id = self.resolve_requester_discord_id(&user_id).await;
        let mut names: Vec<String> = taps
            .into_iter()
            .filter(|t| {
                Self::check_access_resolved(t, &user_id, requester_discord_id.as_deref())
            })
            .map(|t| t.name.0)
            .collect();
        names.sort();
        self.tap_names_cache.set_cached(&user_id, &names).await;
        Ok(names)
    }

    /// Find a tap by name (ASCII case-insensitive) that the user can access, skipping
    /// the per-tap owner/metrics enrichment — 2 queries total. Used by the bot to
    /// validate a user-supplied voice name.
    pub async fn find_accessible_by_name(
        &self,
        user_id: Option<UserId>,
        name: &str,
    ) -> CoreResult<Option<Tap>> {
        let taps = self.tap_repo.list_all().await?;
        let requester_discord_id = self.resolve_requester_discord_id(&user_id).await;
        Ok(taps.into_iter().find(|t| {
            t.name.0.eq_ignore_ascii_case(name)
                && Self::check_access_resolved(t, &user_id, requester_discord_id.as_deref())
        }))
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

        // 3. Resolve the requester once; when only accessible taps are wanted, drop the
        //    rest *before* enrichment so we never pay the per-tap cost for hidden taps.
        let requester_discord_id = self.resolve_requester_discord_id(&user_id).await;
        if accessible == Some(true) {
            taps.retain(|t| {
                Self::check_access_resolved(t, &user_id, requester_discord_id.as_deref())
            });
        }

        // 4. Batch owner + metrics lookups once, then enrich surviving taps concurrently,
        //    carrying the query-free access decision.
        let (owners, rows) = self.batch_owner_and_rows(&taps).await?;
        let mut tap_dtos: Vec<TapWithAccessDto> = stream::iter(taps)
            .map(|tap| {
                let has_access =
                    Self::check_access_resolved(&tap, &user_id, requester_discord_id.as_deref());
                let owner = owners.get(&tap.owner_id).cloned();
                let row = rows.get(&tap.id).cloned();
                async move {
                    let owner = match owner {
                        Some(o) => o,
                        None => {
                            tracing::warn!(tap_id = %tap.id, "skipping tap with missing owner");
                            return None;
                        }
                    };
                    Some(self.enrich_tap(tap, has_access, &owner, row).await)
                }
            })
            .buffer_unordered(ENRICH_CONCURRENCY)
            .filter_map(|x| async move { x })
            .collect()
            .await;

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

        let tap_dto = self.map_to_tap_dto(tap, None).await;

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

    #[tracing::instrument(skip(self), level = "debug", fields(tap_id = %tap_id.0), err)]
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

        // Only a non-owner requester against a whitelist/blacklist needs the discord
        // id; every other case (public, owner-only, owner short-circuit) is
        // query-free, matching the previous behaviour.
        let needs_discord = matches!(
            tap.permission,
            TapPermission::Whitelisted { .. } | TapPermission::Blacklisted { .. }
        ) && user_id.as_ref().is_some_and(|uid| *uid != tap.owner_id);
        let requester_discord_id = if needs_discord {
            self.resolve_requester_discord_id(&user_id).await
        } else {
            None
        };
        Self::check_access_resolved(tap, &user_id, requester_discord_id.as_deref())
    }

    /// Query-free access check. Identical semantics to [`check_access`], but the
    /// requester's discord id is resolved once by the caller instead of being fetched
    /// per tap — this is what makes bulk listing cheap.
    fn check_access_resolved(
        tap: &Tap,
        user_id: &Option<UserId>,
        requester_discord_id: Option<&str>,
    ) -> bool {
        use hq_types::hq::TapPermission;

        match &tap.permission {
            TapPermission::Public => true,
            TapPermission::OwnerOnly => {
                user_id.as_ref().map(|id| *id == tap.owner_id).unwrap_or(false)
            }
            TapPermission::Whitelisted { user_ids } => match user_id {
                Some(uid) if *uid == tap.owner_id => true,
                Some(_) => requester_discord_id
                    .map(|d| user_ids.iter().any(|u| u == d))
                    .unwrap_or(false),
                None => false,
            },
            TapPermission::Blacklisted { user_ids } => match user_id {
                Some(uid) if *uid == tap.owner_id => true,
                Some(_) => requester_discord_id
                    .map(|d| !user_ids.iter().any(|u| u == d))
                    .unwrap_or(true),
                None => true,
            },
        }
    }

    /// Build the DTO for one tap. `prefetched_row` lets bulk callers pass the latest
    /// metrics row they already batch-fetched (see `get_latest_rows`); when `None` the
    /// row is fetched here for single-tap callers.
    async fn map_to_tap_dto(&self, tap: Tap, prefetched_row: Option<TapMetricsRow>) -> TapDto {
        // Fetch the Redis-backed values concurrently. The latest metrics row is either
        // supplied by the caller (bulk path, already batched) or fetched once here.
        let (delta, unique_users, accumulated_uptime, online_states) = tokio::join!(
            self.tap_metrics.redis.peek_delta(&tap.id),
            self.tap_metrics.get_unique_users_count(tap.id.clone()),
            self.tap_metrics.get_uptime_secs(tap.id.clone()),
            self.tap_hub_state.get_tap_states(&tap.id),
        );

        let row = match prefetched_row {
            Some(r) => Some(r),
            None => self.tap_metrics.get_latest_row(&tap.id).await.ok().flatten(),
        };
        let (delta_total, delta_cache) = delta.unwrap_or((0, 0));
        let total_uses =
            (row.as_ref().map(|r| r.total_uses).unwrap_or(0) + delta_total).max(0) as u64;
        let cache_hits =
            (row.as_ref().map(|r| r.cache_hits).unwrap_or(0) + delta_cache).max(0) as u64;
        let unique_users = unique_users.unwrap_or(0);
        let accumulated_uptime = accumulated_uptime.unwrap_or(0);
        let online_states = online_states.unwrap_or_default();
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

#[cfg(test)]
mod access_tests {
    use super::TapService;
    use hq_types::hq::{Tap, TapPermission, UserId};

    fn tap_with(permission: TapPermission) -> Tap {
        let mut t = Tap::new("tap1", "owner1", "name".to_string());
        t.permission = permission;
        t
    }

    fn uid(s: &str) -> Option<UserId> {
        Some(UserId(s.to_string()))
    }

    #[test]
    fn public_is_always_accessible() {
        let t = tap_with(TapPermission::Public);
        assert!(TapService::check_access_resolved(&t, &None, None));
        assert!(TapService::check_access_resolved(&t, &uid("other"), Some("123")));
    }

    #[test]
    fn owner_only_grants_owner_only() {
        let t = tap_with(TapPermission::OwnerOnly);
        assert!(TapService::check_access_resolved(&t, &uid("owner1"), None));
        assert!(!TapService::check_access_resolved(&t, &uid("other"), None));
        assert!(!TapService::check_access_resolved(&t, &None, None));
    }

    #[test]
    fn whitelist_grants_owner_and_members_only() {
        let t = tap_with(TapPermission::Whitelisted {
            user_ids: vec!["123".to_string()],
        });
        // owner always in
        assert!(TapService::check_access_resolved(&t, &uid("owner1"), Some("999")));
        // listed member
        assert!(TapService::check_access_resolved(&t, &uid("other"), Some("123")));
        // non-member
        assert!(!TapService::check_access_resolved(&t, &uid("other"), Some("999")));
        // no user, and user with unresolved discord id → denied
        assert!(!TapService::check_access_resolved(&t, &None, None));
        assert!(!TapService::check_access_resolved(&t, &uid("other"), None));
    }

    #[test]
    fn blacklist_denies_members_only() {
        let t = tap_with(TapPermission::Blacklisted {
            user_ids: vec!["123".to_string()],
        });
        // owner always in even if listed
        assert!(TapService::check_access_resolved(&t, &uid("owner1"), Some("123")));
        // listed member denied
        assert!(!TapService::check_access_resolved(&t, &uid("other"), Some("123")));
        // unlisted allowed
        assert!(TapService::check_access_resolved(&t, &uid("other"), Some("999")));
        // no user, and user with unresolved discord id → allowed
        assert!(TapService::check_access_resolved(&t, &None, None));
        assert!(TapService::check_access_resolved(&t, &uid("other"), None));
    }
}

#[cfg(test)]
mod list_by_user_tests {
    use super::TapService;
    use crate::repo::audit_log::AuditLogWithActor;
    use crate::repo::{AuditLogRepo, TapRepository, UserRepository};
    use crate::service::audit_log::AuditLogService;
    use async_trait::async_trait;
    use hq_types::hq::audit_log::{AuditLog, CreateAuditLogDto};
    use hq_types::hq::settings::PartialUserSettings;
    use hq_types::hq::{Tap, TapId, TapName, User, UserId};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use zako3_metrics::TapMetricsService;
    use zako3_states::{
        CacheRepository, CacheRepositoryRef, TapHubStateService, TapNamesCacheService,
    };

    /// In-memory `CacheRepository`. When `delayed_key` is set, any key containing that
    /// substring sleeps before returning — used to make one tap's enrichment complete
    /// later than the others, so an order test can tell `.buffered` (ordered) apart
    /// from `.buffer_unordered` (completion order).
    #[derive(Default)]
    struct FakeCache {
        store: Mutex<HashMap<String, String>>,
        delayed_key: Option<String>,
        delay: Duration,
    }

    impl FakeCache {
        fn with_delay(marker: &str, delay: Duration) -> Self {
            Self {
                store: Mutex::new(HashMap::new()),
                delayed_key: Some(marker.to_string()),
                delay,
            }
        }

        async fn maybe_delay(&self, key: &str) {
            if let Some(marker) = &self.delayed_key
                && key.contains(marker)
            {
                tokio::time::sleep(self.delay).await;
            }
        }
    }

    #[async_trait]
    impl CacheRepository for FakeCache {
        async fn get(&self, key: &str) -> Option<String> {
            self.maybe_delay(key).await;
            self.store.lock().unwrap().get(key).cloned()
        }
        async fn set(&self, key: &str, value: &str) {
            self.store
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
        }
        async fn set_ex(&self, key: &str, value: &str, _ttl_secs: u64) {
            self.set(key, value).await;
        }
        async fn del(&self, key: &str) {
            self.store.lock().unwrap().remove(key);
        }
        async fn incr(&self, _key: &str) -> zako3_states::Result<i64> {
            Ok(0)
        }
        async fn decr(&self, _key: &str) -> zako3_states::Result<i64> {
            Ok(0)
        }
        async fn incrby(&self, _key: &str, _amount: i64) -> zako3_states::Result<i64> {
            Ok(0)
        }
        async fn pfadd(&self, _key: &str, _element: &str) -> zako3_states::Result<()> {
            Ok(())
        }
        async fn pfcount(&self, _key: &str) -> zako3_states::Result<u64> {
            Ok(0)
        }
        async fn pfcount_multi(&self, _keys: &[String]) -> zako3_states::Result<u64> {
            Ok(0)
        }
        async fn sadd(&self, _key: &str, _member: &str) -> zako3_states::Result<()> {
            Ok(())
        }
        async fn smembers(&self, _key: &str) -> zako3_states::Result<Vec<String>> {
            Ok(vec![])
        }
        async fn hgetall(&self, _key: &str) -> zako3_states::Result<Vec<(String, String)>> {
            Ok(vec![])
        }
        async fn hincrby(
            &self,
            _key: &str,
            _field: &str,
            _amount: i64,
        ) -> zako3_states::Result<i64> {
            Ok(0)
        }
        async fn hdel_key(&self, _key: &str) -> zako3_states::Result<()> {
            Ok(())
        }
    }

    /// In-memory `TapRepository` returning a pre-arranged tap list.
    #[derive(Clone)]
    struct FakeTapRepo {
        taps: Vec<Tap>,
    }

    #[async_trait]
    impl TapRepository for FakeTapRepo {
        async fn create(&self, tap: &Tap) -> crate::CoreResult<Tap> {
            Ok(tap.clone())
        }
        async fn list_by_owner(&self, _owner_id: UserId) -> crate::CoreResult<Vec<Tap>> {
            Ok(self.taps.clone())
        }
        async fn find_by_id(&self, _id: TapId) -> crate::CoreResult<Option<Tap>> {
            Ok(None)
        }
        async fn find_by_name(&self, _name: &TapName) -> crate::CoreResult<Option<Tap>> {
            Ok(None)
        }
        async fn update(&self, tap: &Tap) -> crate::CoreResult<Tap> {
            Ok(tap.clone())
        }
        async fn delete(&self, _id: TapId) -> crate::CoreResult<()> {
            Ok(())
        }
        async fn list_all(&self) -> crate::CoreResult<Vec<Tap>> {
            Ok(self.taps.clone())
        }
        async fn find_by_ids(&self, _ids: Vec<TapId>) -> crate::CoreResult<Vec<Tap>> {
            Ok(vec![])
        }
    }

    /// In-memory `UserRepository` resolving any id to the configured owner.
    #[derive(Clone)]
    struct FakeUserRepo {
        user: User,
    }

    #[async_trait]
    impl UserRepository for FakeUserRepo {
        async fn find_by_discord_id(&self, _discord_id: &str) -> crate::CoreResult<Option<User>> {
            Ok(None)
        }
        async fn create(&self, user: &User) -> crate::CoreResult<User> {
            Ok(user.clone())
        }
        async fn find_by_id(&self, _id: UserId) -> crate::CoreResult<Option<User>> {
            Ok(Some(self.user.clone()))
        }
        async fn find_by_ids(&self, _ids: Vec<UserId>) -> crate::CoreResult<Vec<User>> {
            Ok(vec![])
        }
        async fn list_all(
            &self,
            _page: u32,
            _per_page: u32,
        ) -> crate::CoreResult<(Vec<User>, u64)> {
            Ok((vec![], 0))
        }
        async fn update_permissions(
            &self,
            _id: UserId,
            _permissions: Vec<String>,
        ) -> crate::CoreResult<User> {
            Ok(self.user.clone())
        }
        async fn set_banned_status(&self, _id: UserId, _banned: bool) -> crate::CoreResult<User> {
            Ok(self.user.clone())
        }
        async fn update_oauth_token(
            &self,
            _id: UserId,
            _oauth_token: Option<String>,
        ) -> crate::CoreResult<()> {
            Ok(())
        }
        async fn get_settings(
            &self,
            _id: UserId,
        ) -> crate::CoreResult<Option<PartialUserSettings>> {
            Ok(None)
        }
        async fn save_settings(
            &self,
            _id: UserId,
            settings: &PartialUserSettings,
        ) -> crate::CoreResult<PartialUserSettings> {
            Ok(settings.clone())
        }
    }

    /// In-memory `AuditLogRepo` — `list_by_user` never touches the audit log.
    #[derive(Clone, Default)]
    struct FakeAuditLogRepo;

    #[async_trait]
    impl AuditLogRepo for FakeAuditLogRepo {
        async fn create(&self, dto: &CreateAuditLogDto) -> crate::CoreResult<AuditLog> {
            Ok(AuditLog {
                id: dto.tap_id.clone(),
                tap_id: dto.tap_id.clone(),
                actor_id: dto.actor_id.clone(),
                action_type: dto.action_type.clone(),
                metadata: dto.metadata.clone(),
                created_at: chrono::Utc::now(),
            })
        }
        async fn find_by_tap_id(
            &self,
            _tap_id: String,
            _page: i64,
            _limit: i64,
        ) -> crate::CoreResult<(Vec<AuditLogWithActor>, i64)> {
            Ok((vec![], 0))
        }
    }

    fn test_service(taps: Vec<Tap>, owner: User, cache: FakeCache) -> TapService {
        let cache: CacheRepositoryRef = Arc::new(cache);
        TapService::new(
            Arc::new(FakeTapRepo { taps }),
            Arc::new(FakeUserRepo { user: owner }),
            AuditLogService::new(Arc::new(FakeAuditLogRepo)),
            // No timescale pool → `get_latest_rows` returns an empty map, so every tap
            // exercises the missing-row fallback inside `map_to_tap_dto`.
            TapMetricsService::new(cache.clone(), None, None),
            TapHubStateService::new(cache.clone()),
            TapNamesCacheService::new(cache.clone()),
        )
    }

    #[tokio::test]
    async fn list_by_user_preserves_input_order() {
        let owner = User::new("owner1", "discord-owner1".to_string(), "owner1".to_string());
        let taps = vec![
            Tap::new("tap-slow", "owner1", "Slow".to_string()),
            Tap::new("tap-b", "owner1", "B".to_string()),
            Tap::new("tap-c", "owner1", "C".to_string()),
        ];
        // "tap-slow" enriches slower than the others; `.buffered` must still yield the
        // DTOs in the original repo order. (`buffer_unordered` would emit it last.)
        let svc = test_service(
            taps,
            owner,
            FakeCache::with_delay("tap-slow", Duration::from_millis(30)),
        );
        let resp = svc
            .list_by_user(UserId("owner1".to_string()))
            .await
            .unwrap();
        let ids: Vec<String> = resp.data.iter().map(|d| d.tap.id.clone()).collect();
        assert_eq!(ids, vec!["tap-slow", "tap-b", "tap-c"]);
        assert_eq!(resp.meta.total, 3);
    }

    #[tokio::test]
    async fn list_by_user_zero_metrics_when_no_row() {
        let owner = User::new("owner1", "discord-owner1".to_string(), "owner1".to_string());
        let taps = vec![Tap::new("tap-a", "owner1", "A".to_string())];
        let svc = test_service(taps, owner, FakeCache::default());
        let resp = svc
            .list_by_user(UserId("owner1".to_string()))
            .await
            .unwrap();

        assert_eq!(resp.meta.total, 1);
        assert_eq!(resp.data.len(), 1);
        let dto = &resp.data[0];
        assert!(dto.has_access);
        assert_eq!(dto.owner.username, "owner1");
        // No metrics row and no Redis deltas → everything zeroes out.
        assert_eq!(dto.tap.total_uses, 0);
        assert_eq!(dto.tap.cache_hits, 0);
        assert_eq!(dto.tap.stats.total_uses, 0);
        assert_eq!(dto.tap.stats.cache_hits, 0);
        assert_eq!(dto.tap.stats.unique_users, 0);
    }
}
