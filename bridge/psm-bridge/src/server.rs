//! HTTP server: axum router, Bearer-auth middleware, and route handlers.
//!
//! Every route is wrapped in a Bearer-auth check — there is no unauthenticated
//! route in this API, including `/v1/health`. Phase 1b is read-only:
//! `writes_enabled` is hard-coded `false` regardless of configuration.
//!
//! SECURITY: all endpoint routes are registered in [`app_routes`], the one
//! function `.layer()` in [`router`] wraps. See that function's doc comment
//! before adding a new route.

use std::collections::{BTreeMap, HashMap};
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use axum::extract::{Path, Query, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use uuid::Uuid;

use psm_save::save::containers::PlayerContainerIds;
use psm_save::save::debug::{node_to_json, resolve, DumpOpts};
use psm_save::save::decompress::{decompress_sav, SaveError};
use psm_save::save::gvas::{default_skip_set, parse_gvas};
use psm_save::save::model::{ItemContainer, Pal, Player, PlayerSummary};
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
        .route("/v1/debug/savfiles", get(debug_savfiles))
        .route("/v1/debug/savtree", get(debug_savtree))
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

/// `GET /v1/players/{id}` — full player detail: the character-map summary,
/// level/exp and stat-point allocations (from `Level.sav`), unlocked
/// technologies + points (from the per-player `<uid>.sav`), the player's pals
/// (party/box/base — all their owned pals), the party/pal-box container ids so
/// the client can group them, and their resolved inventory.
///
/// A player whose per-player `.sav` is missing on disk yields empty
/// technologies + inventory rather than an error (the player id is still valid).
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
    let full: Player = bundle
        .players
        .iter()
        .find(|p| p.uid == uid)
        .cloned()
        .unwrap_or_default();

    // Per-player `.sav`: container ids + technologies. Missing file ⇒ defaults.
    let save = match state.app.player_save(&uid).await {
        Ok(s) => s,
        Err(StateError::Load(SaveError::Io(_))) => Default::default(),
        Err(e) => return internal_error(e),
    };

    let pals = pals_owned_by(&bundle, &uid);
    let inventory = resolve_inventory(&bundle, &save.containers);

    Json(PlayerDetail {
        summary,
        level: full.level,
        exp: full.exp,
        status_points: full.status_point_list,
        ext_status_points: full.ext_status_point_list,
        technologies: save.technologies,
        technology_points: save.technology_points,
        boss_technology_points: save.boss_technology_points,
        pal_box_container: save.containers.pal_storage,
        party_container: save.containers.otomo,
        pals,
        inventory,
    })
    .into_response()
}

#[derive(Serialize)]
struct PlayerDetail {
    summary: PlayerSummary,
    level: i32,
    exp: i32,
    /// Stat-point allocations, e.g. `{ "MaxHP": 3, "MaxSP": 2, ... }`.
    status_points: BTreeMap<String, i32>,
    ext_status_points: BTreeMap<String, i32>,
    /// Unlocked technology codes (resolved to names client-side).
    technologies: Vec<String>,
    technology_points: i32,
    boss_technology_points: i32,
    /// Party (`OtomoCharacterContainerId`) and pal-box
    /// (`PalStorageContainerId`) ids, so the client can label a pal's location.
    pal_box_container: String,
    party_container: String,
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

    let save = match state.app.player_save(&uid).await {
        Ok(s) => s,
        Err(StateError::Load(SaveError::Io(_))) => Default::default(),
        Err(e) => return internal_error(e),
    };
    Json(resolve_inventory(&bundle, &save.containers)).into_response()
}

/// Resolve a player's five inventory containers from their (already-read)
/// container ids: look each id up in the cached `bundle.item_containers` (each
/// slot's `dynamic_item` already resolved against `dynamic_items`) and tag it
/// with its role. Unknown/empty ids are skipped, so a player with no matching
/// containers on disk yields an empty vec.
fn resolve_inventory(bundle: &WorldBundle, ids: &PlayerContainerIds) -> Vec<ItemContainer> {
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
    out
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

// ---- Debug: raw GVAS tree viewer (read-only) ----

#[derive(Serialize)]
struct SavFileInfo {
    /// File name, e.g. `Level.sav`.
    name: String,
    /// Path relative to `save_dir`, forward-slashed, e.g. `Players/ABC….sav`.
    rel_path: String,
    size_bytes: u64,
}

/// `GET /v1/debug/savfiles` — every `.sav` under `save_dir` (recursive, bounded),
/// so the viewer can populate its picker without guessing names.
async fn debug_savfiles(State(state): State<ServerState>) -> Response {
    let dir = state.app.save_dir();
    let mut out = Vec::new();
    collect_sav_files(dir, dir, 0, &mut out);
    out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Json(out).into_response()
}

/// Recursively collect `.sav` files under `root`, capped in depth and count so a
/// deep backup tree can't make the listing unbounded.
fn collect_sav_files(
    root: &std::path::Path,
    dir: &std::path::Path,
    depth: usize,
    out: &mut Vec<SavFileInfo>,
) {
    if depth > 4 || out.len() >= 1000 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_sav_files(root, &path, depth + 1, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("sav"))
        {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let rel = path.strip_prefix(root).unwrap_or(&path);
            out.push(SavFileInfo {
                name: path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_string(),
                rel_path: rel.to_string_lossy().replace('\\', "/"),
                size_bytes: size,
            });
        }
        if out.len() >= 1000 {
            return;
        }
    }
}

#[derive(serde::Deserialize)]
struct SavTreeParams {
    /// A name/relative path under `save_dir`, or an absolute path to a `.sav`.
    file: String,
    /// Dotted subtree path; empty = root.
    #[serde(default)]
    path: String,
    page: Option<usize>,
    depth: Option<usize>,
}

/// Failure modes of [`build_savtree`], mapped to HTTP status by the handler.
enum SavTreeError {
    /// Filesystem read failed → 500.
    Io(std::io::Error),
    /// Decompress or GVAS parse failed on a well-formed request → 422.
    Decode(SaveError),
    /// The requested subtree path doesn't exist → 404.
    NoPath,
}

/// Read + decompress + GVAS-parse + project one subtree to JSON, returning the
/// node and the file's byte size. Runs entirely inside the blocking-pool +
/// `catch_unwind` boundary in [`debug_savtree`], because the GVAS reader
/// `assert!`s on malformed input (see `psm_save::save::reader`) — the same
/// quarantine every other decode path uses (`AppState::decode_off_thread`).
fn build_savtree(path: &std::path::Path, sub: &str, opts: DumpOpts) -> Result<(serde_json::Value, usize), SavTreeError> {
    let bytes = std::fs::read(path).map_err(SavTreeError::Io)?;
    let size = bytes.len();
    let raw = decompress_sav(&bytes).map_err(SavTreeError::Decode)?;
    let gvas = parse_gvas(&raw, &default_skip_set()).map_err(SavTreeError::Decode)?;
    let node = resolve(&gvas.root, sub).ok_or(SavTreeError::NoPath)?;
    Ok((node_to_json(&node, &opts), size))
}

/// `GET /v1/debug/savtree?file=&path=&page=&depth=` — one bounded subtree of a
/// `.sav`'s decoded generic GVAS tree. Read-only; byte blobs are summarized.
async fn debug_savtree(State(state): State<ServerState>, Query(p): Query<SavTreeParams>) -> Response {
    let path = match resolve_sav_path(state.app.save_dir(), &p.file) {
        Ok(path) => path,
        Err(detail) => return bad_request(detail),
    };
    let sub = p.path.clone();
    let opts = DumpOpts {
        page: Some(p.page.unwrap_or(200).clamp(1, 2000)),
        depth: p.depth.unwrap_or(2).min(12),
    };

    // Decode off the async runtime, catching any decoder panic on a crafted or
    // corrupt save so it becomes a 422 instead of unwinding the request task.
    let outcome = tokio::task::spawn_blocking(move || {
        std::panic::catch_unwind(AssertUnwindSafe(move || build_savtree(&path, &sub, opts)))
    })
    .await;

    let (node, size) = match outcome {
        Ok(Ok(Ok(v))) => v,
        Ok(Ok(Err(SavTreeError::Io(e)))) => return internal_error(e),
        Ok(Ok(Err(SavTreeError::Decode(e)))) => return unprocessable(e),
        Ok(Ok(Err(SavTreeError::NoPath))) => return not_found(),
        // Decoder panicked (caught) or the blocking task failed to join.
        Ok(Err(_panic)) => return unprocessable("this .sav could not be decoded"),
        Err(_join) => return internal_error("decode task failed"),
    };
    Json(serde_json::json!({
        "file": p.file,
        "path": p.path,
        "node": node,
        "meta": { "size_bytes": size },
    }))
    .into_response()
}

/// Validate + resolve the `file` param to a readable `.sav` path. A relative
/// `file` is joined under `save_dir` and must canonicalize to within it (no
/// `..` traversal); an absolute path is allowed (the bridge is owner-run) but
/// must still be a real, readable `.sav`.
fn resolve_sav_path(save_dir: &std::path::Path, file: &str) -> Result<PathBuf, String> {
    if file.trim().is_empty() {
        return Err("missing file".into());
    }
    let requested = std::path::Path::new(file);
    let is_absolute = requested.is_absolute();
    let candidate = if is_absolute {
        requested.to_path_buf()
    } else {
        save_dir.join(requested)
    };
    let canonical = candidate
        .canonicalize()
        .map_err(|_| format!("no such .sav file: {file}"))?;
    if !canonical.is_file() {
        return Err(format!("not a file: {file}"));
    }
    let is_sav = canonical
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("sav"));
    if !is_sav {
        return Err("only .sav files can be inspected".into());
    }
    if !is_absolute {
        let root = save_dir.canonicalize().map_err(|e| e.to_string())?;
        if !canonical.starts_with(&root) {
            return Err("path escapes the save directory".into());
        }
    }
    Ok(canonical)
}

fn bad_request(detail: impl std::fmt::Display) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorDetailResponse {
            error: "bad request",
            detail: detail.to_string(),
        }),
    )
        .into_response()
}

/// `422` for a file that exists but could not be decompressed or GVAS-parsed —
/// distinct from a `500` (unexpected IO) or `400` (bad request).
fn unprocessable(err: impl std::fmt::Display) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(ErrorDetailResponse {
            error: "unprocessable save",
            detail: err.to_string(),
        }),
    )
        .into_response()
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
