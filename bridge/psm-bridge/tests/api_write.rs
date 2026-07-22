//! Write-endpoint integration tests: the guard ladder (403 disabled, 404
//! unknown target, 422 invalid values) and full end-to-end writes against a
//! temp copy of the `world1` fixture, verified by re-reading through the API.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use psm_bridge::server::router;
use psm_bridge::state::AppState;
use psm_bridge::supervisor::Supervisor;
use tower::ServiceExt;

const WORLD1_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/saves/world1");
const TOKEN: &str = "test-token-0123456789abcdef";
const PLAYER_O_UID: &str = "8c2f1930-0000-0000-0000-000000000000";

/// Copy the world1 fixture into a fresh temp dir (Level.sav + Players/*).
fn temp_world(tag: &str) -> PathBuf {
    let dst = std::env::temp_dir().join(format!("psm-api-write-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(dst.join("Players")).unwrap();
    let src = Path::new(WORLD1_DIR);
    std::fs::copy(src.join("Level.sav"), dst.join("Level.sav")).unwrap();
    for entry in std::fs::read_dir(src.join("Players")).unwrap().flatten() {
        std::fs::copy(entry.path(), dst.join("Players").join(entry.file_name())).unwrap();
    }
    dst
}

fn make_router_at(dir: &Path, allow_writes: bool) -> axum::Router {
    let state = Arc::new(AppState::new(dir.to_path_buf()));
    router(
        state,
        Arc::new(TOKEN.to_string()),
        Arc::new(Supervisor::new(None)),
        allow_writes,
        false,
        None,
    )
}

async fn request(
    app: &axum::Router,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    let body = match body {
        Some(v) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

/// The uid of player O's common (Inventory) container via the API itself.
async fn common_container_id(app: &axum::Router) -> String {
    let (status, json) = request(app, "GET", "/v1/containers", None).await;
    assert_eq!(status, StatusCode::OK);
    let containers = json["containers"].as_array().expect("containers array");
    assert!(!containers.is_empty(), "fixture should expose containers");
    containers
        .iter()
        .find(|c| c["kind"] == "common" && c["owner_uid"] == PLAYER_O_UID)
        .expect("player O has a common container")["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn health_reports_write_state() {
    let app = make_router_at(Path::new(WORLD1_DIR), false);
    let (status, json) = request(&app, "GET", "/v1/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["writes_enabled"], false);
    assert_eq!(json["server_running"], false);
    assert_eq!(json["capabilities"], serde_json::json!(["read"]));

    let app = make_router_at(Path::new(WORLD1_DIR), true);
    let (_, json) = request(&app, "GET", "/v1/health", None).await;
    assert_eq!(json["writes_enabled"], true);
    assert_eq!(json["capabilities"], serde_json::json!(["read", "write"]));
}

#[tokio::test]
async fn containers_listing_labels_player_bags() {
    let app = make_router_at(Path::new(WORLD1_DIR), false);
    let (status, json) = request(&app, "GET", "/v1/containers", None).await;
    assert_eq!(status, StatusCode::OK);
    let containers = json["containers"].as_array().unwrap();
    // Two fixture players with per-player saves → up to 10 bags; player O's
    // five must all be present and labeled.
    let kinds: Vec<&str> = containers
        .iter()
        .filter(|c| c["owner_uid"] == PLAYER_O_UID)
        .map(|c| c["kind"].as_str().unwrap())
        .collect();
    for kind in ["common", "essential", "weapon_loadout", "player_equip_armor", "food_equip"] {
        assert!(kinds.contains(&kind), "missing bag kind {kind}");
    }
    // The common bag holds the fixture's Wood ×77 in slot 0.
    let common = containers
        .iter()
        .find(|c| c["kind"] == "common" && c["owner_uid"] == PLAYER_O_UID)
        .unwrap();
    assert_eq!(common["slots"][0]["static_id"], "Wood");
    assert!(common["used"].as_u64().unwrap() >= 1);
}

#[tokio::test]
async fn writes_disabled_yields_403() {
    let app = make_router_at(Path::new(WORLD1_DIR), false);
    let cid = common_container_id(&app).await;
    let cases = [
        (format!("/v1/containers/{cid}/resize"), serde_json::json!({"slot_num": 50})),
        (format!("/v1/containers/{cid}/slot"), serde_json::json!({"slot_index":0,"static_id":"Stone","count":1})),
        (format!("/v1/players/{PLAYER_O_UID}/edit"), serde_json::json!({"level": 10})),
        (format!("/v1/players/{PLAYER_O_UID}/technologies"), serde_json::json!({"unlock":["Workbench"]})),
    ];
    for (path, body) in cases {
        let (status, json) = request(&app, "POST", &path, Some(body)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        assert_eq!(json["error"], "writes_disabled", "{path}");
    }
}

#[tokio::test]
async fn resize_end_to_end() {
    let dir = temp_world("resize");
    let app = make_router_at(&dir, true);
    let cid = common_container_id(&app).await;

    let (status, json) = request(
        &app,
        "POST",
        &format!("/v1/containers/{cid}/resize"),
        Some(serde_json::json!({"slot_num": 60})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["ok"], true);
    let backup = json["backup"].as_str().unwrap();
    assert!(Path::new(backup).exists(), "backup file exists");
    assert_eq!(json["container"]["slot_num"], 60);

    // Fresh GET reflects the resize (cache self-invalidated by mtime change).
    let (_, json) = request(&app, "GET", "/v1/containers", None).await;
    let c = json["containers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == cid.as_str())
        .unwrap()
        .clone();
    assert_eq!(c["slot_num"], 60);

    std::fs::remove_dir_all(&dir).ok();
}

/// PR #299 (palworld-save-pal) generalizes container resize to the guild chest.
/// Our resize path is container-agnostic — a guild chest is just an
/// `ItemContainerSaveData` entry — so this proves the guild-chest case
/// specifically: list containers, find the `guild_chest`, resize it, and
/// confirm a fresh GET reflects the new slot count. Also emits a diagnostic of
/// the fixture's container kinds + guilds so a fixture without a guild chest is
/// visible rather than silently passing.
#[tokio::test]
async fn guild_chest_resize_end_to_end() {
    let dir = temp_world("guildchest");
    let app = make_router_at(&dir, true);

    let (status, json) = request(&app, "GET", "/v1/containers", None).await;
    assert_eq!(status, StatusCode::OK);
    let containers = json["containers"].as_array().unwrap().clone();
    let kinds: Vec<&str> = containers.iter().map(|c| c["kind"].as_str().unwrap()).collect();
    let (_, guilds) = request(&app, "GET", "/v1/guilds", None).await;
    eprintln!("[guild_chest_resize] container kinds = {kinds:?}");
    eprintln!("[guild_chest_resize] guilds = {guilds}");

    let Some(chest) = containers.iter().find(|c| c["kind"] == "guild_chest") else {
        // Surface the gap loudly: the write path is proven container-agnostic by
        // resize_end_to_end; if world1 has no guild chest we can't exercise it here.
        panic!(
            "world1 fixture exposes no guild_chest container (kinds: {kinds:?}) — \
             cannot prove guild-chest resize end-to-end against this fixture"
        );
    };
    let cid = chest["id"].as_str().unwrap().to_string();
    let orig = chest["slot_num"].as_i64().unwrap();
    let target = if orig == 40 { 55 } else { 40 };

    let (status, json) = request(
        &app,
        "POST",
        &format!("/v1/containers/{cid}/resize"),
        Some(serde_json::json!({ "slot_num": target })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["ok"], true);
    assert_eq!(json["container"]["slot_num"], target);

    // Fresh GET reflects the guild-chest resize.
    let (_, json) = request(&app, "GET", "/v1/containers", None).await;
    let c = json["containers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == cid.as_str())
        .expect("resized guild chest still listed")
        .clone();
    assert_eq!(c["slot_num"], target);
    assert_eq!(c["kind"], "guild_chest");

    std::fs::remove_dir_all(&dir).ok();
}

/// First base under any guild (there is one in world1), as (guild_id, base_json).
async fn first_base(app: &axum::Router) -> serde_json::Value {
    let (_, guilds) = request(app, "GET", "/v1/guilds", None).await;
    guilds
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["bases"].as_array().cloned().unwrap_or_default())
        .next()
        .expect("world1 has a base camp")
}

#[tokio::test]
async fn base_area_edit_end_to_end() {
    let dir = temp_world("basearea");
    let app = make_router_at(&dir, true);

    let base = first_base(&app).await;
    let bid = base["id"].as_str().unwrap().to_string();
    assert_eq!(base["area_range"], 3500.0, "fixture base starts at the vanilla radius");

    // Grow the area (in-place f32) and rename the base (length-changing fstring,
    // Japanese→ASCII shrink) in one edit — exercises both splice paths at once.
    let (status, json) = request(
        &app,
        "POST",
        &format!("/v1/bases/{bid}/edit"),
        Some(serde_json::json!({ "area_range": 7000.0, "name": "Fort Test" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["ok"], true);
    assert!(json["backup"].as_str().is_some(), "backup written");

    let (_, guilds) = request(&app, "GET", "/v1/guilds", None).await;
    let base = guilds
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["bases"].as_array().cloned().unwrap_or_default())
        .find(|b| b["id"] == bid.as_str())
        .unwrap();
    assert_eq!(base["area_range"], 7000.0);
    assert_eq!(base["name"], "Fort Test");

    // Out-of-range radius is rejected (422), not written.
    let (status, _) = request(
        &app,
        "POST",
        &format!("/v1/bases/{bid}/edit"),
        Some(serde_json::json!({ "area_range": 999_999.0 })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn guild_edit_end_to_end() {
    let dir = temp_world("guildedit");
    let app = make_router_at(&dir, true);

    let (_, guilds) = request(&app, "GET", "/v1/guilds", None).await;
    let g0 = guilds.as_array().unwrap()[0].clone();
    let gid = g0["id"].as_str().unwrap().to_string();
    assert_eq!(g0["base_camp_level"], 1);

    let (status, json) = request(
        &app,
        "POST",
        &format!("/v1/guilds/{gid}/edit"),
        Some(serde_json::json!({ "guild_name": "The Test Guild", "base_camp_level": 12 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["ok"], true);

    let (_, guilds) = request(&app, "GET", "/v1/guilds", None).await;
    let g = guilds
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == gid.as_str())
        .unwrap();
    assert_eq!(g["base_camp_level"], 12);
    assert_eq!(g["name"], "The Test Guild");

    // Out-of-range level rejected.
    let (status, _) = request(
        &app,
        "POST",
        &format!("/v1/guilds/{gid}/edit"),
        Some(serde_json::json!({ "base_camp_level": 999 })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn fast_travel_unlock_end_to_end() {
    let dir = temp_world("fasttravel");
    let app = make_router_at(&dir, true);

    // Unlock all fast-travel points for player O. The op re-parses and asserts
    // every bundled id is present + true before returning OK, so a 200 proves
    // the FastTravelPointUnlockFlag map was written (or created) correctly.
    let (status, json) = request(
        &app,
        "POST",
        &format!("/v1/players/{PLAYER_O_UID}/map"),
        Some(serde_json::json!({ "unlock_all_fast_travel": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["ok"], true);

    // Idempotent: a second unlock is a no-op (already fully unlocked) but still OK.
    let (status2, json2) = request(
        &app,
        "POST",
        &format!("/v1/players/{PLAYER_O_UID}/map"),
        Some(serde_json::json!({ "unlock_all_fast_travel": true })),
    )
    .await;
    assert_eq!(status2, StatusCode::OK, "{json2}");

    // An empty request (nothing to do) is rejected.
    let (status3, _) = request(
        &app,
        "POST",
        &format!("/v1/players/{PLAYER_O_UID}/map"),
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(status3, StatusCode::UNPROCESSABLE_ENTITY);

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn base_pal_batch_ops_end_to_end() {
    let dir = temp_world("basepals");
    let app = make_router_at(&dir, true);

    // Enumerate bases: remember the first base (any), and any base that has pals
    // stationed at it (Base.pals is back-filled from each pal's storage_id == the
    // base's WorkerDirector container id).
    let (_, guilds) = request(&app, "GET", "/v1/guilds", None).await;
    let mut any_base: Option<String> = None;
    let mut base_with_pals: Option<String> = None;
    for g in guilds.as_array().unwrap() {
        for b in g["bases"].as_array().cloned().unwrap_or_default() {
            let id = b["id"].as_str().unwrap().to_string();
            let n = b["pals"].as_array().map(|a| a.len()).unwrap_or(0);
            any_base.get_or_insert_with(|| id.clone());
            if n > 0 {
                base_with_pals = Some(id);
            }
        }
    }

    // The work-suitability validator runs BEFORE the pal lookup, so the v0.6.1
    // guard is exercised on any base even when none has pals stationed: a bare
    // enum code (missing the "EPalWorkSuitability::" prefix) must be rejected 422
    // rather than reaching the write layer and corrupting the save.
    if let Some(bid) = &any_base {
        let (status, json) = request(
            &app,
            "POST",
            &format!("/v1/bases/{bid}/pals/edit"),
            Some(serde_json::json!({ "work_suitability": { "Handcraft": 5 } })),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{json}");
        assert!(
            json["detail"]
                .as_str()
                .unwrap_or_default()
                .contains("fully qualified"),
            "bare work-suitability key should be rejected as unqualified, got {json}"
        );
    }

    let Some(bid) = base_with_pals else {
        eprintln!("[base_pals] world1 has no base-stationed pals — batch write not exercised");
        return;
    };

    // Heal all pals at the base (one write).
    let (status, json) = request(
        &app,
        "POST",
        &format!("/v1/bases/{bid}/pals/heal"),
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["ok"], true);

    // Max work affinity for all pals at the base (all 13 suitabilities → 5).
    // Keys are fully qualified, exactly as the frontend sends them.
    let ws = serde_json::json!({ "work_suitability": {
        "EPalWorkSuitability::EmitFlame": 5, "EPalWorkSuitability::Watering": 5,
        "EPalWorkSuitability::Seeding": 5, "EPalWorkSuitability::GenerateElectricity": 5,
        "EPalWorkSuitability::Handcraft": 5, "EPalWorkSuitability::Collection": 5,
        "EPalWorkSuitability::Deforest": 5, "EPalWorkSuitability::Mining": 5,
        "EPalWorkSuitability::OilExtraction": 5, "EPalWorkSuitability::ProductMedicine": 5,
        "EPalWorkSuitability::Cool": 5, "EPalWorkSuitability::Transport": 5,
        "EPalWorkSuitability::MonsterFarm": 5
    }});
    let (status, json) = request(&app, "POST", &format!("/v1/bases/{bid}/pals/edit"), Some(ws)).await;
    assert_eq!(status, StatusCode::OK, "{json}");

    // Level all pals at the base to 30.
    let (status, json) = request(
        &app,
        "POST",
        &format!("/v1/bases/{bid}/pals/edit"),
        Some(serde_json::json!({ "level": 30, "exp": 0 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");

    // An empty edit is rejected.
    let (status, _) = request(
        &app,
        "POST",
        &format!("/v1/bases/{bid}/pals/edit"),
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn slot_set_and_clear_end_to_end() {
    let dir = temp_world("slot");
    let app = make_router_at(&dir, true);
    let cid = common_container_id(&app).await;

    // Overwrite slot 0 (Wood ×77 in the fixture).
    let (status, json) = request(
        &app,
        "POST",
        &format!("/v1/containers/{cid}/slot"),
        Some(serde_json::json!({"slot_index":0,"static_id":"Stone","count":42})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    let slot0 = json["container"]["slots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["slot_index"] == 0)
        .unwrap()
        .clone();
    assert_eq!(slot0["static_id"], "Stone");
    assert_eq!(slot0["count"], 42);

    // Clear it.
    let (status, json) = request(
        &app,
        "POST",
        &format!("/v1/containers/{cid}/slot"),
        Some(serde_json::json!({"slot_index":0,"static_id":"None","count":0})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert!(json["container"]["slots"]
        .as_array()
        .unwrap()
        .iter()
        .all(|s| s["slot_index"] != 0));

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn player_edit_end_to_end() {
    let dir = temp_world("pedit");
    let app = make_router_at(&dir, true);

    let (status, json) = request(
        &app,
        "POST",
        &format!("/v1/players/{PLAYER_O_UID}/edit"),
        Some(serde_json::json!({"level": 55, "exp": 3947260})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["ok"], true);

    let (_, detail) = request(&app, "GET", &format!("/v1/players/{PLAYER_O_UID}"), None).await;
    assert_eq!(detail["level"], 55);
    assert_eq!(detail["exp"], 3947260);

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn pal_edit_end_to_end() {
    let dir = temp_world("paledit");
    let app = make_router_at(&dir, true);

    let (_, pals) = request(&app, "GET", &format!("/v1/players/{PLAYER_O_UID}/pals"), None).await;
    let pal_id = pals[0]["instance_id"].as_str().unwrap().to_string();

    let (status, json) = request(
        &app,
        "POST",
        &format!("/v1/pals/{pal_id}/edit"),
        Some(serde_json::json!({
            "level": 42, "nickname": "Testy",
            "passive_skills": ["Legend", "Rare"],
            "talent_hp": 100, "rank": 3
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");

    let (_, pals) = request(&app, "GET", &format!("/v1/players/{PLAYER_O_UID}/pals"), None).await;
    let pal = pals
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["instance_id"] == pal_id.as_str())
        .unwrap();
    assert_eq!(pal["level"], 42);
    assert_eq!(pal["nickname"], "Testy");
    assert_eq!(pal["passive_skills"], serde_json::json!(["Legend", "Rare"]));
    assert_eq!(pal["talent_hp"], 100);
    assert_eq!(pal["rank"], 3);

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn technologies_end_to_end() {
    let dir = temp_world("tech");
    let app = make_router_at(&dir, true);

    let (status, json) = request(
        &app,
        "POST",
        &format!("/v1/players/{PLAYER_O_UID}/technologies"),
        Some(serde_json::json!({"unlock": ["Workbench", "HandTorch"], "technology_point": 5})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");

    let (_, detail) = request(&app, "GET", &format!("/v1/players/{PLAYER_O_UID}"), None).await;
    let techs = detail["technologies"].as_array().unwrap();
    assert!(techs.iter().any(|t| t == "Workbench"));
    assert!(techs.iter().any(|t| t == "HandTorch"));
    assert_eq!(detail["technology_points"], 5);

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn invalid_targets_and_values() {
    let dir = temp_world("invalid");
    let app = make_router_at(&dir, true);
    let cid = common_container_id(&app).await;
    let ghost = "11111111-2222-3333-4444-555555555555";

    // Unknown ids → 404.
    for (path, body) in [
        (format!("/v1/containers/{ghost}/resize"), serde_json::json!({"slot_num": 10})),
        (format!("/v1/players/{ghost}/edit"), serde_json::json!({"level": 10})),
        (format!("/v1/pals/{ghost}/edit"), serde_json::json!({"level": 10})),
        (format!("/v1/players/{ghost}/technologies"), serde_json::json!({"unlock": ["Workbench"]})),
    ] {
        let (status, _) = request(&app, "POST", &path, Some(body)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }

    // Invalid values → 422.
    for (path, body) in [
        (format!("/v1/containers/{cid}/resize"), serde_json::json!({"slot_num": 10000})),
        (format!("/v1/containers/{cid}/slot"), serde_json::json!({"slot_index": 9999, "static_id": "Stone", "count": 1})),
        (format!("/v1/players/{PLAYER_O_UID}/edit"), serde_json::json!({"level": 0})),
        (format!("/v1/players/{PLAYER_O_UID}/edit"), serde_json::json!({"exp": -5})),
    ] {
        let (status, _) = request(&app, "POST", &path, Some(body)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{path}");
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn clear_container_end_to_end() {
    let dir = temp_world("clear");
    let app = make_router_at(&dir, true);
    let cid = common_container_id(&app).await;

    let (status, json) =
        request(&app, "POST", &format!("/v1/containers/{cid}/clear"), None).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["container"]["used"], 0);
    assert!(json["backup"].is_string(), "a real clear takes one backup");

    // Clearing again is a no-op: still ok, but no file write and no backup.
    let (status, json) =
        request(&app, "POST", &format!("/v1/containers/{cid}/clear"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("backup").is_none() || json["backup"].is_null());

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn pal_heal_delete_clone_end_to_end() {
    let dir = temp_world("tier2");
    let app = make_router_at(&dir, true);

    let (_, pals) = request(&app, "GET", &format!("/v1/players/{PLAYER_O_UID}/pals"), None).await;
    let pal = pals[0].clone();
    let pal_id = pal["instance_id"].as_str().unwrap().to_string();

    // Heal.
    let (status, json) = request(&app, "POST", &format!("/v1/pals/{pal_id}/heal"), None).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    let (_, pals) = request(&app, "GET", &format!("/v1/players/{PLAYER_O_UID}/pals"), None).await;
    let healed = pals
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["instance_id"] == pal_id.as_str())
        .unwrap();
    assert_eq!(healed["sanity"], 100);
    // HP is set to the formula's max (upstream palworld-save-pal parity —
    // slightly under the game's true 1.0 max, which adds friendship bonuses
    // the reference formula predates; the game tops it off on load).
    assert!(healed["hp"].as_i64().unwrap() > 0);

    // Clone into the owner's pal box.
    let (status, json) = request(&app, "POST", &format!("/v1/pals/{pal_id}/clone"), None).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    let clone_id = json["instance_id"].as_str().unwrap().to_string();
    let (_, pals) = request(&app, "GET", &format!("/v1/players/{PLAYER_O_UID}/pals"), None).await;
    let cloned = pals
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["instance_id"] == clone_id.as_str())
        .expect("clone visible via API");
    assert_eq!(cloned["character_id"], pal["character_id"]);

    // Delete the clone.
    let (status, json) =
        request(&app, "POST", &format!("/v1/pals/{clone_id}/delete"), None).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    let (_, pals) = request(&app, "GET", &format!("/v1/players/{PLAYER_O_UID}/pals"), None).await;
    assert!(pals
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["instance_id"] != clone_id.as_str()));

    // Gender + work suitability through the edit endpoint.
    let (status, json) = request(
        &app,
        "POST",
        &format!("/v1/pals/{pal_id}/edit"),
        Some(serde_json::json!({
            "gender": "Female",
            "work_suitability": {"EPalWorkSuitability::Cool": 2}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    let (_, pals) = request(&app, "GET", &format!("/v1/players/{PLAYER_O_UID}/pals"), None).await;
    let edited = pals
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["instance_id"] == pal_id.as_str())
        .unwrap();
    assert_eq!(edited["gender"], "EPalGenderType::Female");
    assert_eq!(edited["work_suitability"]["EPalWorkSuitability::Cool"], 2);

    // Invalid values → 422.
    for body in [
        serde_json::json!({"gender": "Yes"}),
        serde_json::json!({"work_suitability": {"EPalWorkSuitability::Cool": 9}}),
    ] {
        let (status, _) =
            request(&app, "POST", &format!("/v1/pals/{pal_id}/edit"), Some(body)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    std::fs::remove_dir_all(&dir).ok();
}
