use crate::repo::{TapRepository, VerificationRepository};
use crate::service::validation::validate_verification_request;
use crate::service::AuditLogService;
use crate::{CoreError, CoreResult};
use hq_types::hq::{
    CreateVerificationRequestDto, TapId, UserId, VerificationRequest, VerificationRequestId,
    VerificationStatus,
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
        tap_id: TapId,
        user_id: UserId,
        dto: CreateVerificationRequestDto,
    ) -> CoreResult<VerificationRequest> {
        validate_verification_request(&dto.title, &dto.description)?;
        let tap = self
            .tap_repo
            .find_by_id(tap_id.clone())
            .await?
            .ok_or_else(|| CoreError::NotFound("Tap not found".to_string()))?;

        if tap.owner_id != user_id {
            return Err(CoreError::Forbidden(
                "You do not have permission to request verification for this tap".to_string(),
            ));
        }

        // Check for existing pending request
        if self
            .verification_repo
            .find_pending_by_tap_id(tap_id.clone())
            .await?
            .is_some()
        {
            return Err(CoreError::InvalidInput(
                "A verification request is already pending for this tap".to_string(),
            ));
        }

        if tap.occupation != hq_types::hq::TapOccupation::Base {
            return Err(CoreError::InvalidInput(
                "Congrats! Your tap is already verified!".to_string(),
            ));
        }

        let id = hq_types::hq::next_id().to_string();
        let now = chrono::Utc::now();
        let request = VerificationRequest {
            id: VerificationRequestId(id),
            tap_id: tap_id.clone(),
            tap: Some(tap),
            requester_id: user_id.clone(),
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
                tap_id.0,
                Some(user_id.0),
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
        let (mut requests, total) = self
            .verification_repo
            .list_all(status, page, per_page)
            .await?;

        // Attach taps
        let tap_ids: Vec<TapId> = requests
            .iter()
            .map(|r| r.tap_id.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        if !tap_ids.is_empty() {
            let taps = self.tap_repo.find_by_ids(tap_ids).await?;
            let tap_map: std::collections::HashMap<TapId, hq_types::hq::Tap> =
                taps.into_iter().map(|t| (t.id.clone(), t)).collect();

            for request in &mut requests {
                request.tap = tap_map.get(&request.tap_id).cloned();
            }
        }

        Ok((requests, total))
    }

    pub async fn approve_verification(
        &self,
        request_id: VerificationRequestId,
        admin_id: UserId,
    ) -> CoreResult<VerificationRequest> {
        let request = self
            .verification_repo
            .find_by_id(request_id.clone())
            .await?
            .ok_or_else(|| CoreError::NotFound("Verification request not found".to_string()))?;

        if request.status != VerificationStatus::Pending {
            return Err(CoreError::InvalidInput(
                "Only pending requests can be approved".to_string(),
            ));
        }

        // 1. Update request status
        let mut updated_request = self
            .verification_repo
            .update_status(request_id, VerificationStatus::Approved, None)
            .await?;

        // 2. Update Tap occupation
        let mut tap = self
            .tap_repo
            .find_by_id(request.tap_id.clone())
            .await?
            .ok_or_else(|| CoreError::NotFound("Tap not found".to_string()))?;

        tap.occupation = hq_types::hq::TapOccupation::Verified;
        tap.timestamp.updated_at = chrono::Utc::now();
        self.tap_repo.update(&tap).await?;

        // Attach tap to response
        updated_request.tap = Some(tap);

        // 3. Log
        let _ = self
            .audit_log
            .log(
                request.tap_id.0,
                Some(admin_id.0),
                "tap.verification_approved".to_string(),
                None,
            )
            .await;

        Ok(updated_request)
    }

    pub async fn reject_verification(
        &self,
        request_id: VerificationRequestId,
        admin_id: UserId,
        reason: String,
    ) -> CoreResult<VerificationRequest> {
        let request = self
            .verification_repo
            .find_by_id(request_id.clone())
            .await?
            .ok_or_else(|| CoreError::NotFound("Verification request not found".to_string()))?;

        if request.status != VerificationStatus::Pending {
            return Err(CoreError::InvalidInput(
                "Only pending requests can be rejected".to_string(),
            ));
        }

        let mut updated_request = self
            .verification_repo
            .update_status(
                request_id,
                VerificationStatus::Rejected,
                Some(reason.clone()),
            )
            .await?;

        // Fetch and attach tap
        let tap = self.tap_repo.find_by_id(request.tap_id.clone()).await?;
        updated_request.tap = tap;

        let _ = self
            .audit_log
            .log(
                request.tap_id.0,
                Some(admin_id.0),
                "tap.verification_rejected".to_string(),
                Some(serde_json::json!({ "reason": reason })),
            )
            .await;

        Ok(updated_request)
    }
}
