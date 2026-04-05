use crate::CoreResult;
use crate::repo::audit_log::AuditLogRepo;
use hq_types::hq::audit_log::{ActorDto, AuditLogDto, CreateAuditLogDto, PaginatedAuditLogsDto};
use hq_types::hq::dtos::{PaginationMetaDto, UserSummaryDto};
use std::sync::Arc;

#[derive(Clone)]
pub struct AuditLogService {
    repo: Arc<dyn AuditLogRepo>,
}

impl AuditLogService {
    pub fn new(repo: Arc<dyn AuditLogRepo>) -> Self {
        Self { repo }
    }

    pub async fn log(
        &self,
        tap_id: u64,
        actor_id: Option<u64>,
        action_type: String,
        metadata: Option<serde_json::Value>,
    ) -> CoreResult<()> {
        let dto = CreateAuditLogDto {
            tap_id,
            actor_id,
            action_type,
            metadata,
        };
        self.repo.create(&dto).await?;
        Ok(())
    }

    pub async fn get_tap_logs(
        &self,
        tap_id: u64,
        page: i64,
        limit: i64,
    ) -> CoreResult<PaginatedAuditLogsDto> {
        let (records, total) = self.repo.find_by_tap_id(tap_id, page, limit).await?;

        let data = records
            .into_iter()
            .map(|r| {
                let actor = if let (Some(actor_id), Some(username)) = (r.log.actor_id, r.username) {
                    ActorDto::User(UserSummaryDto {
                        id: actor_id.to_string(),
                        username,
                        avatar: r.avatar_url.unwrap_or_default(),
                    })
                } else {
                    ActorDto::System
                };

                AuditLogDto {
                    id: r.log.id.to_string(),
                    tap_id: r.log.tap_id.to_string(),
                    actor,
                    action_type: r.log.action_type,
                    metadata: r.log.metadata,
                    created_at: r.log.created_at,
                }
            })
            .collect();

        let total_pages = if total > 0 {
            ((total as f64) / (limit as f64)).ceil() as u64
        } else {
            0
        };

        Ok(PaginatedAuditLogsDto {
            data,
            meta: PaginationMetaDto {
                total: total as u64,
                page: page as u64,
                per_page: limit as u64,
                total_pages,
            },
        })
    }
}
