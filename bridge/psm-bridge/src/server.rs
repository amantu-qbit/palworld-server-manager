//! HTTP server: axum router, Bearer-auth middleware, and route handlers.
//!
//! Every route is wrapped in a Bearer-auth check — there is no unauthenticated
//! route in this API, including `/v1/health`. Phase 1b is read-only:
//! `writes_enabled` is hard-coded `false` regardless of configuration.
//!
//! SECURITY: all endpoint routes are registered in [`app_routes`], the one
//! function `.layer()` in [`router`] wraps. See that function's doc comment
//! before adding a new route.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use axum::extract::{Path, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use uuid::Uuid;

use psm_save::save::decompress::SaveError;
use psm_save::save::model::{ItemContainer, Pal, PlayerSummary};
use psm_save::save::reference::{load_reference, Reference};
use psm_save::save::WorldBundle;

use crate::state::{AppState, StateError};
use crate::supervisor::{ServerStatus, Supervisor, SupervisorError};

/// Combined router state: the decoded-save cache plus the configured auth
/// token. Cheap to clone — both fields are `Arc`s.
#[derive(Clone)]
struct ServerState {
    app: Arc<AppState>,
    token: Arc<String>,
    supervisor: Arc<Supervisor>,
}

/// SECURITY: EVERY endpoint route MUST be added inside this function.
///
/// `router()` applies the Bearer-auth layer to exactly the routes registered
/// here. A route registered anywhere else (e.g. merged into the `Router`
/// returned by `router()` after `.layer()` has already been applied) is
/// UNAUTHENTICATED — axum's `.layer()` only wraps routes added to the
/// `Router` *before* the `.layer()` call, not routes added after.
fn app_routes() -> Router<ServerState> {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/players", get(list_players))
        .route("/v1/players/{id}", get(player_detail))
        .route("/v1/players/{id}/pals", get(player_pals))
        .route("/v1/players/{id}/inventory", get(player_inventory))
        .route("/v1/guilds", get(list_guilds))
        .route("/v1/reference/{catalog}", get(reference_catalog))
        .route("/v1/server/status", get(server_status))
        .route("/v1/server/start", post(server_start))
        .route("/v1/server/stop", post(server_stop))
        .route("/v1/server/restart", post(server_restart))
}

/// Build the bridge HTTP router.
///
/// Currently exposes `GET /v1/health`. Every route registered in
/// [`app_routes`] is wrapped in a Bearer-auth layer (see [`auth`]) that
/// requires the `Authorization` header to equal `Bearer {token}`.
pub fn router(state: Arc<AppState>, token: Arc<String>, supervisor: Arc<Supervisor>) -> Router {
    let server_state = ServerState {
        app: state,
        token,
        supervisor,
    };

    app_routes()
        // SECURITY: this layer only wraps routes registered in `app_routes()`
        // above. Do not add `.route(...)` calls below this line — they would
        // be unauthenticated.
        .layer(middleware::from_fn_with_state(server_state.clone(), auth))
        .with_state(server_state)
}

#[derive(Serialize)]
struct HealthResponse {
    version: &'static str,
    capabilities: &'static [&'static str],
    save_detected: bool,
    writes_enabled: bool,
}

/// `GET /v1/health` — reports server version, capabilities, and whether a
/// save file is currently detected. Writes are always disabled in Phase 1b.
async fn health(State(state): State<ServerState>) -> impl IntoResponse {
    Json(HealthResponse {
        version: env!("CARGO_PKG_VERSION"),
        capabilities: &["read"],
        save_detected: state.app.level_sav_exists(),
        writes_enabled: false,
    })
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse {
            error: "unauthorized",
        }),
    )
        .into_response()
}

/// `404 { "error": "not found" }` — an unrecognized player uid or reference
/// catalog name.
fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse { error: "not found" }),
    )
        .into_response()
}

#[derive(Serialize)]
struct ErrorDetailResponse {
    error: &'static str,
    detail: String,
}

/// `500` for a decode/IO failure surfaced from [`AppState`] (a corrupt save,
/// a filesystem error, or a caught decoder panic) — distinct from
/// [`not_found`], which is for a well-formed request whose target simply
/// doesn't exist.
fn internal_error(err: impl std::fmt::Display) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorDetailResponse {
            error: "internal error",
            detail: err.to_string(),
        }),
    )
        .into_response()
}

/// Normalize a path-param uid to the same canonical (lowercase, hyphenated)
/// form the decoder stores on `PlayerSummary::uid` / `Pal::owner_uid`, so
/// comparisons are exact-string but case/format-insensitive on input. `None`
/// for anything that isn't a valid UUID at all — callers treat that the same
/// as "no such player" (404), since a malformed id can never match a real one.
fn canonical_uid(id: &str) -> Option<String> {
    Uuid::parse_str(id).ok().map(|u| u.to_string())
}

/// `GET /v1/players` — every character in `Level.sav` (online and offline).
async fn list_players(State(state): State<ServerState>) -> Response {
    match state.app.bundle().await {
        Ok(bundle) => Json(bundle.world.players.clone()).into_response(),
        Err(e) => internal_error(e),
    }
}

/// `GET /v1/players/{id}` — composed player detail: the character-map
/// summary, the player's pals, and their resolved inventory.
///
/// NOTE (deferred): full player detail per the design doc — stats,
/// technologies + points, missions, fast-travel/effigies — is **not**
/// included. Phase 1a only decodes the "lite" player fields carried by the
/// `CharacterSaveParameterMap` entry (identity/nickname/level/vitals) plus
/// the container ids; decoding the rest of a player's own `<uid>.sav` is an
/// explicitly deferred decoder task (see the Task 6 brief), so this endpoint
/// composes only what Phase 1a already has rather than fabricating the
/// missing fields.
async fn player_detail(State(state): State<ServerState>, Path(id): Path<String>) -> Response {
    let bundle = match state.app.bundle().await {
        Ok(bundle) => bundle,
        Err(e) => return internal_error(e),
    };
    let Some(uid) = canonical_uid(&id) else {
        return not_found();
    };
    let Some(summary) = bundle.world.players.iter().find(|p| p.uid == uid).cloned() else {
        return not_found();
    };
    let pals = pals_owned_by(&bundle, &uid);
    let inventory = match resolve_inventory(&state.app, &bundle, &uid).await {
        Ok(inventory) => inventory,
        Err(e) => return internal_error(e),
    };

    Json(PlayerDetail {
        summary,
        pals,
        inventory,
    })
    .into_response()
}

#[derive(Serialize)]
struct PlayerDetail {
    summary: PlayerSummary,
    pals: Vec<Pal>,
    inventory: Vec<ItemContainer>,
}

/// `GET /v1/players/{id}/pals` — that player's pals (`World.pals` filtered by
/// `owner_uid == id`). `404` if `id` is not a known player uid.
async fn player_pals(State(state): State<ServerState>, Path(id): Path<String>) -> Response {
    let bundle = match state.app.bundle().await {
        Ok(bundle) => bundle,
        Err(e) => return internal_error(e),
    };
    let Some(uid) = canonical_uid(&id) else {
        return not_found();
    };
    if !bundle.world.players.iter().any(|p| p.uid == uid) {
        return not_found();
    }

    Json(pals_owned_by(&bundle, &uid)).into_response()
}

fn pals_owned_by(bundle: &WorldBundle, uid: &str) -> Vec<Pal> {
    bundle
        .world
        .pals
        .iter()
        .filter(|pal| pal.owner_uid == uid)
        .cloned()
        .collect()
}

/// `GET /v1/players/{id}/inventory` — the player's five inventory containers
/// (Common/Essential/WeaponLoadOut/PlayerEquipArmor/FoodEquip). `404` if `id`
/// is not a known player uid.
async fn player_inventory(State(state): State<ServerState>, Path(id): Path<String>) -> Response {
    let bundle = match state.app.bundle().await {
        Ok(bundle) => bundle,
        Err(e) => return internal_error(e),
    };
    let Some(uid) = canonical_uid(&id) else {
        return not_found();
    };
    if !bundle.world.players.iter().any(|p| p.uid == uid) {
        return not_found();
    }

    match resolve_inventory(&state.app, &bundle, &uid).await {
        Ok(inventory) => Json(inventory).into_response(),
        Err(e) => internal_error(e),
    }
}

/// Resolve a known player's five inventory containers: reads their per-player
/// `<UID>.sav` for the container ids (`AppState::player_container_ids`), then
/// looks each id up in the cached `bundle.item_containers` (already decoded
/// with each slot's `dynamic_item` resolved against `dynamic_items` — see
/// `psm_save::save::containers::decode_item_containers`). The returned
/// containers are tagged with their `container_type` (the decoder itself
/// doesn't know which of the five roles a given container id plays; the
/// caller does, from which `PlayerContainerIds` field resolved it).
///
/// Design choice (Task 6 brief, "pick one, note it"): a *known* player whose
/// per-player `.sav` is missing from disk yields an empty inventory
/// (`Ok(vec![])`), not a 404 — the player id itself is valid, only the
/// on-disk container-id lookup came up empty. `404` is reserved for an
/// unrecognized player id, checked by the caller before this is reached.
async fn resolve_inventory(
    app: &AppState,
    bundle: &WorldBundle,
    uid: &str,
) -> Result<Vec<ItemContainer>, StateError> {
    let ids = match app.player_container_ids(uid).await {
        Ok(ids) => ids,
        Err(StateError::Load(SaveError::Io(_))) => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };

    let container_ids: [(&str, &str); 5] = [
        ("Common", ids.common.as_str()),
        ("Essential", ids.essential.as_str()),
        ("WeaponLoadOut", ids.weapon_loadout.as_str()),
        ("PlayerEquipArmor", ids.player_equip_armor.as_str()),
        ("FoodEquip", ids.food_equip.as_str()),
    ];

    let mut out = Vec::with_capacity(container_ids.len());
    for (container_type, id) in container_ids {
        let Ok(container_uuid) = Uuid::parse_str(id) else {
            continue;
        };
        if let Some(container) = bundle.item_containers.get(&container_uuid) {
            let mut container = container.clone();
            container.container_type = container_type.to_string();
            out.push(container);
        }
    }
    Ok(out)
}

/// `GET /v1/guilds` — every guild in `Level.sav`.
async fn list_guilds(State(state): State<ServerState>) -> Response {
    match state.app.bundle().await {
        Ok(bundle) => Json(bundle.world.guilds.clone()).into_response(),
        Err(e) => internal_error(e),
    }
}

/// Process-wide cache of the vendored reference catalogs: they're static
/// (compiled in via `include_str!`), so parsing them once per process
/// (rather than per request) avoids repeated JSON parsing on a hot path.
static REFERENCE: OnceLock<Reference> = OnceLock::new();

fn reference() -> &'static Reference {
    REFERENCE.get_or_init(load_reference)
}

/// `GET /v1/reference/{catalog}` — the vendored id -> display-name catalog
/// for `catalog` ∈ `{items, active_skills, passive_skills, elements}`. `404`
/// for any other catalog name.
async fn reference_catalog(Path(catalog): Path<String>) -> Response {
    let map: &HashMap<String, String> = match catalog.as_str() {
        "items" => reference().items(),
        "active_skills" => reference().active_skills(),
        "passive_skills" => reference().passive_skills(),
        "elements" => reference().elements(),
        _ => return not_found(),
    };
    Json(map).into_response()
}

// --- Server process control (supervisor) ------------------------------------
//
// These endpoints start / stop / restart the game server process. They write
// no save files. Graceful shutdown (warn + save) is the desktop app's job via
// Palworld's REST `/shutdown`; `stop`/`restart` here are force operations.

/// `GET /v1/server/status` — whether the supervised server is running.
async fn server_status(State(state): State<ServerState>) -> Response {
    Json(state.supervisor.status()).into_response()
}

/// `POST /v1/server/start` — launch the configured server if not already running.
async fn server_start(State(state): State<ServerState>) -> Response {
    run_supervised(state.supervisor.clone(), Supervisor::start).await
}

/// `POST /v1/server/stop` — force-stop the server (kills its whole process tree).
async fn server_stop(State(state): State<ServerState>) -> Response {
    run_supervised(state.supervisor.clone(), Supervisor::stop).await
}

/// `POST /v1/server/restart` — force stop, then start.
async fn server_restart(State(state): State<ServerState>) -> Response {
    run_supervised(state.supervisor.clone(), Supervisor::restart).await
}

/// Run a (blocking) supervisor operation off the async executor and map its
/// result to an HTTP response. `start`/`stop`/`restart` spawn a process or
/// shell out to `taskkill`, so they must not run on a tokio worker thread.
async fn run_supervised(
    supervisor: Arc<Supervisor>,
    op: impl FnOnce(&Supervisor) -> Result<ServerStatus, SupervisorError> + Send + 'static,
) -> Response {
    match tokio::task::spawn_blocking(move || op(&supervisor)).await {
        Ok(result) => supervisor_result(result),
        Err(_join_error) => internal_error("server control task failed"),
    }
}

/// Map a supervisor result to a JSON response with an appropriate status code.
fn supervisor_result(result: Result<ServerStatus, SupervisorError>) -> Response {
    match result {
        Ok(status) => Json(status).into_response(),
        Err(err) => {
            let code = match err {
                SupervisorError::NotConfigured => StatusCode::BAD_REQUEST,
                SupervisorError::AlreadyRunning(_) | SupervisorError::NotRunning => {
                    StatusCode::CONFLICT
                }
                SupervisorError::Launch(_) | SupervisorError::Stop(_) => {
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            };
            (
                code,
                Json(ErrorDetailResponse {
                    error: "server control error",
                    detail: err.to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// Bearer-auth middleware, applied to every route via [`router`].
///
/// Requires an `Authorization: Bearer <token>` header whose token equals the
/// configured token exactly, using a constant-time comparison so response
/// timing does not leak how many leading bytes of a guessed token were
/// correct. Missing header, malformed header, or a mismatched token all
/// produce `401 {"error": "unauthorized"}`.
async fn auth(State(state): State<ServerState>, request: Request, next: Next) -> Response {
    let presented = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    match presented {
        Some(token) if constant_time_eq(token.as_bytes(), state.token.as_bytes()) => {
            next.run(request).await
        }
        _ => unauthorized(),
    }
}

/// Constant-time byte-slice equality check.
///
/// Compares lengths first (a length mismatch is not secret information, and
/// short-circuiting on it doesn't leak anything an attacker doesn't already
/// know from the token format). For equal-length inputs, folds every byte
/// pair with XOR-OR rather than returning early on the first mismatch, so
/// the loop always runs the full length and the timing does not reveal how
/// many leading bytes matched.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_matches_equal_slices() {
        assert!(constant_time_eq(b"same-token", b"same-token"));
    }

    #[test]
    fn constant_time_eq_rejects_different_same_length_slices() {
        assert!(!constant_time_eq(b"token-aaaa", b"token-bbbb"));
    }

    #[test]
    fn constant_time_eq_rejects_different_length_slices() {
        assert!(!constant_time_eq(b"short", b"a-much-longer-token"));
    }

    #[test]
    fn constant_time_eq_treats_empty_slices_as_equal() {
        assert!(constant_time_eq(b"", b""));
    }
}
