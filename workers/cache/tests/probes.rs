use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use zako3_cache::server::{self, WarmupState};
use zako3_preload_cache::{AudioPreload, FileAudioCache};

async fn router_with(
    dir: &tempfile::TempDir,
    admin_token: Option<&str>,
    warmup: Arc<WarmupState>,
) -> Router {
    let cache = Arc::new(
        FileAudioCache::open_empty(dir.path().to_path_buf(), None)
            .await
            .expect("open cache"),
    );
    let preload = Arc::new(AudioPreload::new(dir.path().to_path_buf(), None));
    server::build(
        cache,
        preload,
        admin_token.map(str::to_string),
        warmup,
    )
}

async fn get(router: Router, path: &str) -> (StatusCode, String) {
    let res = router
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("route request");
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("read body");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// The kubelet carries no admin token. Before this, `/healthz` sat under the
/// auth layer and returned 401 the moment a token was configured, crash-looping
/// the pod.
#[tokio::test]
async fn healthz_answers_without_the_admin_token() {
    let dir = tempfile::tempdir().unwrap();
    let router = router_with(&dir, Some("s3cret"), Arc::new(WarmupState::new())).await;

    let (status, body) = get(router, "/healthz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn healthz_answers_while_the_index_is_still_warming() {
    let dir = tempfile::tempdir().unwrap();
    let warmup = Arc::new(WarmupState::new());
    let router = router_with(&dir, None, Arc::clone(&warmup)).await;

    assert!(!warmup.is_ready());
    let (status, _) = get(router, "/healthz").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the pod must stay Ready during warmup so it keeps taking traffic"
    );
}

#[tokio::test]
async fn readyz_reports_warmup_progress_then_flips() {
    let dir = tempfile::tempdir().unwrap();
    let warmup = Arc::new(WarmupState::new());

    warmup.record_progress(120, 400);
    let router = router_with(&dir, Some("s3cret"), Arc::clone(&warmup)).await;
    let (status, body) = get(router, "/readyz").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("\"scanned\":120"), "body was {body}");
    assert!(body.contains("\"total\":400"), "body was {body}");
    assert!(body.contains("\"ready\":false"), "body was {body}");

    warmup.mark_ready();
    let router = router_with(&dir, Some("s3cret"), Arc::clone(&warmup)).await;
    let (status, body) = get(router, "/readyz").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"ready\":true"), "body was {body}");
}

/// The probes being public must not have opened up the data routes.
#[tokio::test]
async fn data_routes_still_require_the_admin_token() {
    let dir = tempfile::tempdir().unwrap();
    let router = router_with(&dir, Some("s3cret"), Arc::new(WarmupState::new())).await;

    let (status, _) = get(router, "/entry?tap_id=t&key=k").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn wait_ready_resolves_once_marked() {
    let warmup = Arc::new(WarmupState::new());
    let waiter = {
        let warmup = Arc::clone(&warmup);
        tokio::spawn(async move { warmup.wait_ready().await })
    };
    warmup.mark_ready();
    tokio::time::timeout(std::time::Duration::from_secs(5), waiter)
        .await
        .expect("wait_ready should resolve after mark_ready")
        .expect("waiter task");

    // Already-ready state resolves immediately.
    tokio::time::timeout(std::time::Duration::from_secs(5), warmup.wait_ready())
        .await
        .expect("wait_ready should return immediately when already ready");
}
