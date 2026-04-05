use crate::repo::NotificationRepository;
use crate::{CoreError, CoreResult};
use hq_types::hq::{CreateNotificationDto, Notification, NotificationDto, PaginatedResponseDto, PaginationMetaDto};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct NotificationService {
    repo: Arc<dyn NotificationRepository>,
}

impl NotificationService {
    pub fn new(repo: Arc<dyn NotificationRepository>) -> Self {
        Self { repo }
    }

    pub async fn create(&self, dto: CreateNotificationDto) -> CoreResult<Notification> {
        let user_id = Uuid::parse_str(&dto.user_id)
            .map_err(|_| CoreError::InvalidInput("Invalid user ID".to_string()))?;
        let n = Notification::new(
            Uuid::new_v4(),
            user_id,
            dto.r#type,
            dto.title,
            dto.message,
        );
        self.repo.create(&n).await
    }

    pub async fn list_by_user(&self, user_id: Uuid) -> CoreResult<PaginatedResponseDto<NotificationDto>> {
        let notifications = self.repo.list_by_user(user_id).await?;
        let dtos: Vec<NotificationDto> = notifications.into_iter().map(|n| NotificationDto {
            id: n.id.0.to_string(),
            user_id: n.user_id.0.to_string(),
            r#type: n.r#type,
            title: n.title,
            message: n.message,
            read_at: n.read_at,
            created_at: n.created_at,
        }).collect();

        let total = dtos.len() as u64;
        Ok(PaginatedResponseDto {
            data: dtos,
            meta: PaginationMetaDto {
                total,
                page: 1,
                per_page: 50,
                total_pages: 1,
            },
        })
    }

    pub async fn mark_as_read(&self, id: Uuid, user_id: Uuid) -> CoreResult<NotificationDto> {
        let n = self.repo.mark_as_read(id, user_id).await?
            .ok_or_else(|| CoreError::NotFound("Notification not found".to_string()))?;

        Ok(NotificationDto {
            id: n.id.0.to_string(),
            user_id: n.user_id.0.to_string(),
            r#type: n.r#type,
            title: n.title,
            message: n.message,
            read_at: n.read_at,
            created_at: n.created_at,
        })
    }
}
