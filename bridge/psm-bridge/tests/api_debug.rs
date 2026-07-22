//! Integration tests for the read-only Raw Save debug endpoints
//! (`/v1/debug/savfiles`, `/v1/debug/savtree`), driven in-process via
//! `tower::ServiceExt::oneshot`.

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
    router(state, Arc::new(TOKEN.to_string()), Arc::new(Supervisor::new(None)), false, false, None)
}

async fn get(uri: &str) -> (StatusCode, String) {
    let app = make_router();
    let request = Request::builder()
        .uri(uri)
        .header("Authorization", format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .expect("build request");
    let response = app.oneshot(request).await.expect("request should succeed");
    let status = response.status();
    let bytes = response.into_body().collect().await.expect("collect body").to_bytes();
    (status, String::from_utf8(bytes.to_vec()).expect("utf8"))
}

#[tokio::test]
async fn savfiles_lists_level_sav() {
    let (status, body) = get("/v1/debug/savfiles").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Level.sav"), "should list Level.sav, got: {body}");
    assert!(body.contains("\"size_bytes\""));
}

#[tokio::test]
async fn savtree_root_exposes_world_save_data() {
    let (status, body) = get("/v1/debug/savtree?file=Level.sav").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("worldSaveData"), "root should include worldSaveData");
    // Byte blobs are summarized, never dumped as raw arrays.
    assert!(body.contains("_bytes"), "expected summarized byte blobs");
}

#[tokio::test]
async fn savtree_drills_into_character_map() {
    let (status, body) =
        get("/v1/debug/savtree?file=Level.sav&path=worldSaveData.CharacterSaveParameterMap").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Map<"), "CharacterSaveParameterMap should render as a Map, got: {body}");
    assert!(body.contains("_count"));
}

#[tokio::test]
async fn savtree_unknown_file_is_bad_request() {
    let (status, _) = get("/v1/debug/savtree?file=NoSuch.sav").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn savtree_non_sav_traversal_is_rejected() {
    // Escapes save_dir to a real file whose extension is not `.sav`.
    let (status, _) = get("/v1/debug/savtree?file=../../../../Cargo.toml").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn savtree_bad_path_is_not_found() {
    let (status, _) = get("/v1/debug/savtree?file=Level.sav&path=NoSuchKey").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn savtree_garbage_sav_is_unprocessable_not_a_crash() {
    // A `.sav` that isn't a valid PlZ/PlM container: decode must fail cleanly as
    // 422, and the request must return (not unwind the task / hang the server).
    let mut p = std::env::temp_dir();
    p.push(format!("psm_debug_garbage_{}.sav", std::process::id()));
    std::fs::write(&p, b"not a real palworld save file at all").expect("write temp .sav");
    // Encode the absolute path for the query (colon on Windows drive letters).
    let file = p.to_str().unwrap().replace('\\', "/").replace(':', "%3A");
    let (status, _) = get(&format!("/v1/debug/savtree?file={file}")).await;
    let _ = std::fs::remove_file(&p);
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn debug_routes_require_auth() {
    let app = make_router();
    let request = Request::builder()
        .uri("/v1/debug/savfiles")
        .body(Body::empty())
        .expect("build request");
    let response = app.oneshot(request).await.expect("request should succeed");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
