use crate::repo::audit_log::AuditLogRepo;
use crate::CoreResult;
use hq_types::hq::audit_log::{AuditLogDto, CreateAuditLogDto, PaginatedAuditLogsDto};
use hq_types::hq::dtos::PaginationMetaDto;
use std::sync::Arc;
use uuid::Uuid;

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
        tap_id: Uuid,
        actor_id: Uuid,
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
        tap_id: Uuid,
        page: i64,
        limit: i64,
    ) -> CoreResult<PaginatedAuditLogsDto> {
        let (records, total) = self.repo.find_by_tap_id(tap_id, page, limit).await?;

        let data = records
            .into_iter()
            .map(|r| AuditLogDto {
                id: r.id.to_string(),
                tap_id: r.tap_id.to_string(),
                actor_id: r.actor_id.to_string(),
                action_type: r.action_type,
                metadata: r.metadata,
                created_at: r.created_at,
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
