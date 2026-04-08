use crate::repo::NotificationRepository;
use crate::{CoreError, CoreResult};
use hq_types::hq::{
    CreateNotificationDto, Notification, NotificationDto, NotificationId, PaginatedResponseDto,
    PaginationMetaDto, UnreadCountDto, UserId,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct NotificationService {
    repo: Arc<dyn NotificationRepository>,
}

impl NotificationService {
    pub fn new(repo: Arc<dyn NotificationRepository>) -> Self {
        Self { repo }
    }

    pub async fn create(&self, dto: CreateNotificationDto) -> CoreResult<Notification> {
        let n = Notification::new(
            hq_types::hq::next_id().to_string(),
            dto.user_id,
            dto.r#type,
            dto.title,
            dto.message,
        );
        self.repo.create(&n).await
    }

    pub async fn list_by_user(
        &self,
        user_id: UserId,
    ) -> CoreResult<PaginatedResponseDto<NotificationDto>> {
        let notifications = self.repo.list_by_user(user_id).await?;
        let dtos: Vec<NotificationDto> = notifications
            .into_iter()
            .map(|n| NotificationDto {
                id: n.id.0.clone(),
                user_id: n.user_id.0.clone(),
                r#type: n.r#type,
                title: n.title,
                message: n.message,
                read_at: n.read_at,
                created_at: n.created_at,
            })
            .collect();

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

    pub async fn mark_as_read(
        &self,
        id: NotificationId,
        user_id: UserId,
    ) -> CoreResult<NotificationDto> {
        let n = self
            .repo
            .mark_as_read(id, user_id)
            .await?
            .ok_or_else(|| CoreError::NotFound("Notification not found".to_string()))?;

        Ok(NotificationDto {
            id: n.id.0.clone(),
            user_id: n.user_id.0.clone(),
            r#type: n.r#type,
            title: n.title,
            message: n.message,
            read_at: n.read_at,
            created_at: n.created_at,
        })
    }

    pub async fn get_unread_count(&self, user_id: UserId) -> CoreResult<UnreadCountDto> {
        let count = self.repo.unread_count(user_id).await?;
        Ok(UnreadCountDto { count })
    }
}
