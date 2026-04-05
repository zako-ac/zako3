use crate::repo::{TapRepository, VerificationRepository};
use crate::service::AuditLogService;
use crate::{CoreError, CoreResult};
use hq_types::hq::{
    CreateVerificationRequestDto, TapId, TapOccupation, UserId, VerificationRequest,
    VerificationRequestId, VerificationStatus,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct VerificationService {
    verification_repo: Arc<dyn VerificationRepository>,
    tap_repo: Arc<dyn TapRepository>,
    audit_log: AuditLogService,
}

impl VerificationService {
    pub fn new(
        verification_repo: Arc<dyn VerificationRepository>,
        tap_repo: Arc<dyn TapRepository>,
        audit_log: AuditLogService,
    ) -> Self {
        Self {
            verification_repo,
            tap_repo,
            audit_log,
        }
    }

    pub async fn request_verification(
        &self,
        tap_id: u64,
        user_id: u64,
        dto: CreateVerificationRequestDto,
    ) -> CoreResult<VerificationRequest> {
        let tap = self
            .tap_repo
            .find_by_id(tap_id)
            .await?
            .ok_or_else(|| CoreError::NotFound("Tap not found".to_string()))?;

        if tap.owner_id.0 != user_id {
            return Err(CoreError::Forbidden(
                "You do not have permission to request verification for this tap".to_string(),
            ));
        }

        // Check for existing pending request
        if self
            .verification_repo
            .find_pending_by_tap_id(tap_id)
            .await?
            .is_some()
        {
            return Err(CoreError::InvalidInput(
                "A verification request is already pending for this tap".to_string(),
            ));
        }

        let id = hq_types::hq::next_id();
        let now = chrono::Utc::now();
        let request = VerificationRequest {
            id: VerificationRequestId(id),
            tap_id: TapId(tap_id),
            requester_id: UserId(user_id),
            title: dto.title,
            description: dto.description,
            status: VerificationStatus::Pending,
            rejection_reason: None,
            created_at: now,
            updated_at: now,
        };

        let created = self.verification_repo.create(&request).await?;

        let _ = self
            .audit_log
            .log(
                tap_id,
                Some(user_id),
                "tap.verification_requested".to_string(),
                None,
            )
            .await;

        Ok(created)
    }

    pub async fn list_requests(
        &self,
        status: Option<VerificationStatus>,
        page: u32,
        per_page: u32,
    ) -> CoreResult<(Vec<VerificationRequest>, u64)> {
        self.verification_repo
            .list_all(status, page, per_page)
            .await
    }

    pub async fn approve_verification(
        &self,
        request_id: u64,
        admin_id: u64,
    ) -> CoreResult<VerificationRequest> {
        let request = self
            .verification_repo
            .find_by_id(request_id)
            .await?
            .ok_or_else(|| CoreError::NotFound("Verification request not found".to_string()))?;

        if request.status != VerificationStatus::Pending {
            return Err(CoreError::InvalidInput(
                "Only pending requests can be approved".to_string(),
            ));
        }

        // 1. Update request status
        let updated_request = self
            .verification_repo
            .update_status(request_id, VerificationStatus::Approved, None)
            .await?;

        // 2. Update Tap occupation
        let mut tap = self
            .tap_repo
            .find_by_id(request.tap_id.0)
            .await?
            .ok_or_else(|| CoreError::NotFound("Tap not found".to_string()))?;

        tap.occupation = TapOccupation::Verified;
        tap.timestamp.updated_at = chrono::Utc::now();
        self.tap_repo.update(&tap).await?;

        // 3. Log
        let _ = self
            .audit_log
            .log(
                request.tap_id.0,
                Some(admin_id),
                "tap.verification_approved".to_string(),
                None,
            )
            .await;

        Ok(updated_request)
    }

    pub async fn reject_verification(
        &self,
        request_id: u64,
        admin_id: u64,
        reason: String,
    ) -> CoreResult<VerificationRequest> {
        let request = self
            .verification_repo
            .find_by_id(request_id)
            .await?
            .ok_or_else(|| CoreError::NotFound("Verification request not found".to_string()))?;

        if request.status != VerificationStatus::Pending {
            return Err(CoreError::InvalidInput(
                "Only pending requests can be rejected".to_string(),
            ));
        }

        let updated_request = self
            .verification_repo
            .update_status(
                request_id,
                VerificationStatus::Rejected,
                Some(reason.clone()),
            )
            .await?;

        let _ = self
            .audit_log
            .log(
                request.tap_id.0,
                Some(admin_id),
                "tap.verification_rejected".to_string(),
                Some(serde_json::json!({ "reason": reason })),
            )
            .await;

        Ok(updated_request)
    }
}
