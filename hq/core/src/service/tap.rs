use crate::repo::{TapRepository, UserRepository};
use crate::{CoreError, CoreResult};
use hq_types::hq::{
    CreateTapDto, PaginatedResponseDto, PaginationMetaDto, Tap, TapDto, TapStatsDto,
    TapWithAccessDto, TimeSeriesPointDto, UserSummaryDto,
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct TapService {
    tap_repo: Arc<dyn TapRepository>,
    user_repo: Arc<dyn UserRepository>,
}

impl TapService {
    pub fn new(tap_repo: Arc<dyn TapRepository>, user_repo: Arc<dyn UserRepository>) -> Self {
        Self {
            tap_repo,
            user_repo,
        }
    }

    pub async fn create(&self, owner_id: Uuid, dto: CreateTapDto) -> CoreResult<Tap> {
        let mut tap = Tap::new(Uuid::new_v4(), owner_id, dto.name);
        tap.description = dto.description;
        self.tap_repo.create(&tap).await
    }

    pub async fn list_by_user(
        &self,
        user_id: Uuid,
    ) -> CoreResult<PaginatedResponseDto<TapWithAccessDto>> {
        let taps = self.tap_repo.list_by_owner(user_id).await?;
        let user = self
            .user_repo
            .find_by_id(user_id)
            .await?
            .ok_or(CoreError::NotFound("User not found".to_string()))?;

        let mut tap_dtos = Vec::new();
        for tap in taps {
            let tap_dto = TapDto {
                id: tap.id.0.to_string(),
                name: tap.name.0.clone(),
                description: tap.description.clone().unwrap_or_default(),
                owner_id: tap.owner_id.0.to_string(),
                occupation: tap.occupation.clone(),
                permission: tap.permission.clone(),
                roles: tap.role.clone().into_iter().collect(),
                total_uses: 0,
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
        tap_id: Uuid,
        user_id: Uuid,
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

        let tap_dto = TapDto {
            id: tap.id.0.to_string(),
            name: tap.name.0.clone(),
            description: tap.description.clone().unwrap_or_default(),
            owner_id: tap.owner_id.0.to_string(),
            occupation: tap.occupation.clone(),
            permission: tap.permission.clone(),
            roles: tap.role.clone().into_iter().collect(),
            total_uses: 0,
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

    pub async fn get_tap_stats(&self, tap_id: Uuid, user_id: Uuid) -> CoreResult<TapStatsDto> {
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

        // Return mock data for now since we don't have analytics tracking yet
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
            currently_active: 5,
            total_uses: 1337,
            cache_hits: 1100,
            unique_users: 42,
            use_rate_history,
            cache_hit_rate_history,
        })
    }
}
