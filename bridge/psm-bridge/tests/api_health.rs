//! Integration test: `GET /v1/health` and the Bearer-auth middleware.
//!
//! Drives the router in-process via `tower::ServiceExt::oneshot` — no real
//! network socket, no bound port.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use psm_bridge::server::router;
use psm_bridge::state::AppState;
use psm_bridge::supervisor::Supervisor;
use tower::ServiceExt;

const WORLD1_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/saves/world1");
const TOKEN: &str = "test-token-0123456789abcdef";

fn make_router() -> axum::Router {
    let state = Arc::new(AppState::new(PathBuf::from(WORLD1_DIR)));
    router(state, Arc::new(TOKEN.to_string()), Arc::new(Supervisor::new(None)), false, None)
}

#[tokio::test]
async fn health_with_correct_token_returns_200_and_save_detected() {
    let app = make_router();
    let request = Request::builder()
        .uri("/v1/health")
        .header("Authorization", format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .expect("build request");

    let response = app.oneshot(request).await.expect("request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect response body")
        .to_bytes();
    let body_str = String::from_utf8(body.to_vec()).expect("response body should be utf8");
    assert!(
        body_str.contains("\"save_detected\""),
        "body should contain save_detected, got: {body_str}"
    );
}

#[tokio::test]
async fn health_with_wrong_token_is_unauthorized() {
    let app = make_router();
    let request = Request::builder()
        .uri("/v1/health")
        .header("Authorization", "Bearer wrong-token-entirely")
        .body(Body::empty())
        .expect("build request");

    let response = app.oneshot(request).await.expect("request should succeed");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn health_with_no_authorization_header_is_unauthorized() {
    let app = make_router();
    let request = Request::builder()
        .uri("/v1/health")
        .body(Body::empty())
        .expect("build request");

    let response = app.oneshot(request).await.expect("request should succeed");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// Regression test for the auth-bypass footgun: axum's `.layer()` only
/// wraps routes registered before the call. This asserts the current
/// guarantee that no path — known or unknown — is reachable without a
/// token, so a future route accidentally added outside `app_routes()`
/// (see `server.rs`) trips this test instead of silently shipping
/// unauthenticated.
#[tokio::test]
async fn no_path_is_reachable_without_a_token() {
    for path in ["/v1/health", "/v1/does-not-exist"] {
        let app = make_router();
        let request = Request::builder()
            .uri(path)
            .body(Body::empty())
            .expect("build request");

        let response = app.oneshot(request).await.expect("request should succeed");

        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "path {path} should require auth, got {}",
            response.status()
        );
    }
}

#[tokio::test]
async fn health_with_non_bearer_authorization_scheme_is_unauthorized() {
    let app = make_router();
    let request = Request::builder()
        .uri("/v1/health")
        .header("Authorization", "Basic Zm9v")
        .body(Body::empty())
        .expect("build request");

    let response = app.oneshot(request).await.expect("request should succeed");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
