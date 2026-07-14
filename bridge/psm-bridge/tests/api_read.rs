//! Integration tests: the Phase-1b read endpoints (players, player detail,
//! inventory, pals, guilds, reference), driven in-process via
//! `tower::ServiceExt::oneshot` against the world1-fixture-backed router.
//!
//! Ground truth for these assertions matches `bridge/tests/decode_world1.rs`:
//! world1 has 2 players (uid `8c2f1930-...` a.k.a. "O", and `43797f87-...`
//! a.k.a. "Sky"), 11 pals all owned by O (including a level-65 JetDragon),
//! and O's common inventory container holds 77x Wood.

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
const PLAYER_O_UID: &str = "8c2f1930-0000-0000-0000-000000000000";

fn make_router() -> axum::Router {
    let state = Arc::new(AppState::new(PathBuf::from(WORLD1_DIR)));
    router(state, Arc::new(TOKEN.to_string()), Arc::new(Supervisor::new(None)))
}

/// GET `path` with the correct Bearer token, returning (status, body-as-utf8).
async fn get(path: &str) -> (StatusCode, String) {
    let app = make_router();
    let request = Request::builder()
        .uri(path)
        .header("Authorization", format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .expect("build request");

    let response = app.oneshot(request).await.expect("request should succeed");
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect response body")
        .to_bytes();
    let body_str = String::from_utf8(body.to_vec()).expect("response body should be utf8");
    (status, body_str)
}

#[tokio::test]
async fn list_players_returns_both_world1_players() {
    let (status, body) = get("/v1/players").await;

    assert_eq!(status, StatusCode::OK);
    let players: serde_json::Value = serde_json::from_str(&body).expect("valid json");
    let players = players.as_array().expect("players is a json array");
    assert_eq!(players.len(), 2, "world1 has exactly 2 players, got: {body}");
    assert!(
        body.contains(PLAYER_O_UID),
        "players list should include player O's uid, got: {body}"
    );
}

#[tokio::test]
async fn player_pals_returns_player_os_pals_including_jetdragon() {
    let (status, body) = get(&format!("/v1/players/{PLAYER_O_UID}/pals")).await;

    assert_eq!(status, StatusCode::OK);
    let pals: serde_json::Value = serde_json::from_str(&body).expect("valid json");
    let pals = pals.as_array().expect("pals is a json array");
    assert_eq!(pals.len(), 11, "player O owns all 11 pals, got: {body}");
    assert!(
        body.contains("JetDragon"),
        "player O's pals should include a JetDragon, got: {body}"
    );
}

#[tokio::test]
async fn player_inventory_contains_wood_slot() {
    let (status, body) = get(&format!("/v1/players/{PLAYER_O_UID}/inventory")).await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("\"static_id\":\"Wood\""),
        "player O's inventory should contain a Wood slot, got: {body}"
    );
}

#[tokio::test]
async fn player_detail_composes_summary_pals_and_inventory() {
    let (status, body) = get(&format!("/v1/players/{PLAYER_O_UID}")).await;

    assert_eq!(status, StatusCode::OK);
    let detail: serde_json::Value = serde_json::from_str(&body).expect("valid json");
    assert_eq!(
        detail["summary"]["uid"].as_str(),
        Some(PLAYER_O_UID),
        "detail.summary.uid should be player O, got: {body}"
    );
    assert_eq!(
        detail["pals"].as_array().expect("pals array").len(),
        11,
        "detail.pals should list all 11 of O's pals, got: {body}"
    );
    assert!(
        body.contains("\"static_id\":\"Wood\""),
        "detail.inventory should contain a Wood slot, got: {body}"
    );
    // Full player detail: level from Level.sav, technologies from the per-player .sav.
    assert_eq!(
        detail["level"].as_i64(),
        Some(65),
        "detail.level should be O's level 65, got: {body}"
    );
    let techs = detail["technologies"].as_array().expect("technologies array");
    assert!(
        !techs.is_empty(),
        "a level-65 player should have unlocked technologies decoded from their .sav, got: {body}"
    );
    assert!(
        techs.iter().all(|t| t.is_string()),
        "technologies should be a list of code strings, got: {body}"
    );
    assert!(
        detail["pal_box_container"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "detail.pal_box_container should be set for grouping, got: {body}"
    );
}

#[tokio::test]
async fn guilds_list_is_non_empty() {
    let (status, body) = get("/v1/guilds").await;

    assert_eq!(status, StatusCode::OK);
    let guilds: serde_json::Value = serde_json::from_str(&body).expect("valid json");
    assert!(
        !guilds.as_array().expect("guilds is a json array").is_empty(),
        "world1 has at least one guild, got: {body}"
    );
}

#[tokio::test]
async fn reference_items_resolves_and_contains_wood() {
    let (status, body) = get("/v1/reference/items").await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("\"Wood\""),
        "items catalog should contain Wood, got a body of length {}",
        body.len()
    );
}

#[tokio::test]
async fn reference_active_skills_resolves() {
    let (status, body) = get("/v1/reference/active_skills").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());
}

#[tokio::test]
async fn reference_passive_skills_resolves() {
    let (status, body) = get("/v1/reference/passive_skills").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());
}

#[tokio::test]
async fn reference_elements_resolves() {
    let (status, body) = get("/v1/reference/elements").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());
}

#[tokio::test]
async fn unknown_player_uid_is_not_found() {
    let (status, body) = get("/v1/players/00000000-1111-2222-3333-444444444444").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
}

#[tokio::test]
async fn unknown_player_uid_pals_is_not_found() {
    let (status, body) = get("/v1/players/00000000-1111-2222-3333-444444444444/pals").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
}

#[tokio::test]
async fn unknown_player_uid_inventory_is_not_found() {
    let (status, body) = get("/v1/players/00000000-1111-2222-3333-444444444444/inventory").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
}

#[tokio::test]
async fn unknown_reference_catalog_is_not_found() {
    let (status, body) = get("/v1/reference/bogus").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
}

/// Spot-check: one of the new routes still requires the Bearer token (the
/// full route-vs-auth-layer regression is already covered exhaustively by
/// `api_health.rs::no_path_is_reachable_without_a_token`).
#[tokio::test]
async fn players_list_without_token_is_unauthorized() {
    let app = make_router();
    let request = Request::builder()
        .uri("/v1/players")
        .body(Body::empty())
        .expect("build request");

    let response = app.oneshot(request).await.expect("request should succeed");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
