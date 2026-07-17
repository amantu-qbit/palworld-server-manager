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
/// token. Cheap to clone — the fields are `Arc`s or `Copy`.
#[derive(Clone)]
struct ServerState {
    app: Arc<AppState>,
    token: Arc<String>,
    supervisor: Arc<Supervisor>,
    /// The `[safety] allow_writes` config value at bind time. Save-write
    /// endpoints 403 when false; `/v1/health` reports it.
    allow_writes: bool,
    /// Serializes save-file writes. Two concurrent write requests would
    /// otherwise read the same original bytes, race the shared `.psm-tmp`
    /// path, and last-write-wins away one of the edits; holding this across
    /// the whole read-edit-backup-replace sequence makes writes sequential.
    write_lock: Arc<tokio::sync::Mutex<()>>,
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
        .route("/v1/players/{id}/edit", post(player_edit))
        .route("/v1/players/{id}/technologies", post(player_technologies))
        .route("/v1/players/{id}/map", post(player_map))
        .route("/v1/pals/{id}/edit", post(pal_edit))
        .route("/v1/pals/{id}/heal", post(pal_heal))
        .route("/v1/pals/{id}/delete", post(pal_delete))
        .route("/v1/pals/{id}/clone", post(pal_clone))
        .route("/v1/guilds", get(list_guilds))
        .route("/v1/guilds/{id}/edit", post(guild_edit))
        .route("/v1/bases/{id}/edit", post(base_edit))
        .route("/v1/bases/{id}/pals/heal", post(base_pals_heal))
        .route("/v1/bases/{id}/pals/edit", post(base_pals_edit))
        .route("/v1/containers", get(list_containers))
        .route("/v1/containers/{id}/resize", post(container_resize))
        .route("/v1/containers/{id}/slot", post(container_slot))
        .route("/v1/containers/{id}/clear", post(container_clear))
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
pub fn router(
    state: Arc<AppState>,
    token: Arc<String>,
    supervisor: Arc<Supervisor>,
    allow_writes: bool,
) -> Router {
    let server_state = ServerState {
        app: state,
        token,
        supervisor,
        allow_writes,
        write_lock: Arc::new(tokio::sync::Mutex::new(())),
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
    /// True when the game server process is detected — save writes are
    /// blocked while it runs (the game holds the save in memory and would
    /// overwrite any edit on its next autosave).
    server_running: bool,
}

/// `GET /v1/health` — server version, capabilities, whether a save file is
/// detected, and whether save writes are currently possible.
async fn health(State(state): State<ServerState>) -> impl IntoResponse {
    Json(HealthResponse {
        version: env!("CARGO_PKG_VERSION"),
        capabilities: if state.allow_writes {
            &["read", "write"]
        } else {
            &["read"]
        },
        save_detected: state.app.level_sav_exists(),
        writes_enabled: state.allow_writes,
        server_running: state.supervisor.status().running,
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
    exp: i64,
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
///
/// Takes the save-write lock first, so a start cannot slip into the window
/// between a write handler's guard check and its file replacement (and vice
/// versa: an in-flight edit finishes before the game server boots and reads
/// the save).
async fn server_start(State(state): State<ServerState>) -> Response {
    let _no_writes_in_flight = state.write_lock.lock().await;
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

// --- Containers + save writes (Phase 2) --------------------------------------
//
// Save-write requests run through a fixed guard ladder — 403 unless
// `allow_writes` is configured on, 409 while the game server process is
// running (it holds the save in memory; an edit would be overwritten by its
// next autosave, or worse interleave with one), 404 for unknown targets,
// 422 for invalid values — and every successful write returns the path of the
// timestamped backup taken before the file was replaced. The mtime/size cache
// key in `AppState` makes the post-write re-read decode fresh state
// automatically.

/// One labeled item container: a player inventory bag or a guild chest.
#[derive(Serialize)]
struct ContainerInfo {
    id: String,
    kind: &'static str,
    label: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner_uid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    guild_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    guild_name: Option<String>,
    slot_num: i32,
    /// The container type's vanilla default slot count, when reliably known — so
    /// the UI can flag a container that's been resized off its default and show
    /// what that default was. `None` for types whose default varies (base
    /// storage chests differ by tier) or isn't reliably known.
    #[serde(skip_serializing_if = "Option::is_none")]
    default_slot_num: Option<i32>,
    used: usize,
    slots: Vec<psm_save::save::model::ItemContainerSlot>,
}

/// The vanilla default slot count for a labeled container kind, when it's a
/// reliable single value. Player-bag values are the established game defaults;
/// the guild chest is 54 (per the wiki). `essential` (key items) and
/// `base_storage` (chest tier varies) have no single default, so they return
/// `None` and the UI makes no "resized" claim for them.
fn default_slots(kind: &str) -> Option<i32> {
    match kind {
        "common" => Some(42),
        "weapon_loadout" => Some(4),
        "player_equip_armor" => Some(2),
        "food_equip" => Some(3),
        "guild_chest" => Some(54),
        _ => None,
    }
}

#[derive(Serialize)]
struct ContainersResponse {
    containers: Vec<ContainerInfo>,
}

/// Accessor picking one bag's container id off a player's id set.
type BagIdAccessor = fn(&PlayerContainerIds) -> &String;

/// The five player-bag kinds, in display order, with their
/// `PlayerContainerIds` accessor.
const BAG_KINDS: [(&str, &str, BagIdAccessor); 5] = [
    ("common", "Inventory", |c| &c.common),
    ("essential", "Key Items", |c| &c.essential),
    ("weapon_loadout", "Weapons", |c| &c.weapon_loadout),
    ("player_equip_armor", "Armor", |c| &c.player_equip_armor),
    ("food_equip", "Food", |c| &c.food_equip),
];

fn container_info_from(
    bundle: &WorldBundle,
    id: Uuid,
    kind: &'static str,
    label: &'static str,
) -> Option<ContainerInfo> {
    let c = bundle.item_containers.get(&id)?;
    let used = c
        .slots
        .iter()
        .filter(|s| !s.static_id.is_empty() && s.static_id != "None")
        .count();
    Some(ContainerInfo {
        id: id.to_string(),
        kind,
        label,
        owner_uid: None,
        owner_name: None,
        guild_id: None,
        guild_name: None,
        slot_num: c.slot_num,
        default_slot_num: default_slots(kind),
        used,
        slots: c.slots.clone(),
    })
}

/// Every labeled container in the world: each player's five bags plus each
/// guild's chest. Players whose per-player `.sav` is missing are skipped.
async fn collect_containers(state: &ServerState) -> Result<Vec<ContainerInfo>, Response> {
    let bundle = state.app.bundle().await.map_err(internal_error)?;
    let mut out = Vec::new();

    for p in &bundle.world.players {
        let save = match state.app.player_save(&p.uid).await {
            Ok(s) => s,
            // A missing, truncated, or corrupt per-player save must not fail
            // the whole listing — skip that player's bags; everyone else's
            // containers stay reachable.
            Err(_) => continue,
        };
        for (kind, label, id_of) in BAG_KINDS {
            let Ok(cid) = Uuid::parse_str(id_of(&save.containers)) else {
                continue;
            };
            if let Some(mut info) = container_info_from(&bundle, cid, kind, label) {
                info.owner_uid = Some(p.uid.clone());
                info.owner_name = Some(p.nickname.clone());
                out.push(info);
            }
        }
    }

    for g in &bundle.world.guilds {
        let Ok(gid) = Uuid::parse_str(&g.id) else {
            continue;
        };
        if let Some(cid) = bundle.guild_chests.get(&gid) {
            if let Some(mut info) = container_info_from(&bundle, *cid, "guild_chest", "Guild Chest") {
                info.guild_id = Some(g.id.clone());
                info.guild_name = Some(g.name.clone());
                out.push(info);
            }
        }
        // Built storage chests at each of the guild's bases. The frontend maps
        // these back to a base by matching the id against `base.storage_containers`.
        for b in &g.bases {
            for cid_str in &b.storage_containers {
                let Ok(cid) = Uuid::parse_str(cid_str) else {
                    continue;
                };
                if out.iter().any(|c| c.id == *cid_str) {
                    continue; // never list the same container twice
                }
                if let Some(mut info) =
                    container_info_from(&bundle, cid, "base_storage", "Base Storage")
                {
                    info.guild_id = Some(g.id.clone());
                    info.guild_name = Some(g.name.clone());
                    out.push(info);
                }
            }
        }
    }
    Ok(out)
}

/// `GET /v1/containers` — every labeled item container (player bags + guild
/// chests) with resolved slots.
async fn list_containers(State(state): State<ServerState>) -> Response {
    match collect_containers(&state).await {
        Ok(containers) => Json(ContainersResponse { containers }).into_response(),
        Err(resp) => resp,
    }
}

/// The 403/409 guard ladder every save-write endpoint runs first.
fn write_guard(state: &ServerState) -> Option<Response> {
    if !state.allow_writes {
        return Some(
            (
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: "writes_disabled",
                }),
            )
                .into_response(),
        );
    }
    if state.supervisor.status().running {
        return Some(
            (
                StatusCode::CONFLICT,
                Json(ErrorDetailResponse {
                    error: "server_running",
                    detail: "stop the server before editing saves".to_string(),
                }),
            )
                .into_response(),
        );
    }
    None
}

/// Run a blocking save edit off the async executor with the same
/// `catch_unwind` quarantine as every other GVAS codec path, mapping errors
/// to HTTP responses. Holds the state's write lock for the whole
/// read-edit-backup-replace sequence so concurrent write requests are
/// strictly sequential (each sees the previous write's result). The
/// server-running guard is re-checked *under the lock* — the handler's
/// earlier `write_guard` sample races a concurrent `/v1/server/start`
/// (which also takes this lock) — and the decode cache is explicitly
/// invalidated after a successful write, because the `(mtime, size)` cache
/// key alone cannot distinguish a same-second, same-size rewrite.
async fn run_edit<F>(
    state: &ServerState,
    op: F,
) -> Result<psm_save::save::edit::EditReceipt, Response>
where
    F: FnOnce() -> Result<psm_save::save::edit::EditReceipt, SaveError> + Send + 'static,
{
    let _serialized = state.write_lock.lock().await;
    if state.supervisor.status().running {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorDetailResponse {
                error: "server_running",
                detail: "the server was started while this edit was queued".to_string(),
            }),
        )
            .into_response());
    }
    // Belt-and-suspenders: before the FIRST edit of this bridge session, take a
    // one-shot snapshot of the whole save folder (on top of the per-file backup
    // every edit makes). If it can't be written, refuse the edit — no edit ever
    // proceeds without a full pre-edit backup. Serialized by the write lock, so
    // it runs exactly once.
    if let Err(e) = state.app.ensure_full_backup().await {
        return Err(internal_error(e));
    }
    let outcome =
        tokio::task::spawn_blocking(move || std::panic::catch_unwind(AssertUnwindSafe(op))).await;
    match outcome {
        Ok(Ok(Ok(receipt))) => {
            state.app.invalidate();
            Ok(receipt)
        }
        Ok(Ok(Err(SaveError::Io(e)))) => Err(internal_error(e)),
        Ok(Ok(Err(e))) => Err(unprocessable(e)),
        Ok(Err(_panic)) => Err(unprocessable("save could not be edited")),
        Err(_join) => Err(internal_error("edit task failed")),
    }
}

/// Successful-write response shell.
#[derive(Serialize)]
struct WriteResponse {
    ok: bool,
    /// Backup path; absent when the request was a no-op and no file was
    /// touched.
    #[serde(skip_serializing_if = "Option::is_none")]
    backup: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    container: Option<ContainerInfo>,
}

fn write_ok(receipt: &psm_save::save::edit::EditReceipt, container: Option<ContainerInfo>) -> Response {
    Json(WriteResponse {
        ok: true,
        backup: receipt.backup.as_ref().map(|b| b.display().to_string()),
        container,
    })
    .into_response()
}

/// Re-read the (self-invalidated) bundle and rebuild one container's info,
/// preserving its labeling from a pre-write snapshot.
async fn refreshed_container(
    state: &ServerState,
    cid: Uuid,
) -> Option<ContainerInfo> {
    let all = collect_containers(state).await.ok()?;
    all.into_iter().find(|c| c.id == cid.to_string())
}

#[derive(serde::Deserialize)]
struct GuildEditBody {
    #[serde(default)]
    guild_name: Option<String>,
    #[serde(default)]
    base_camp_level: Option<i32>,
}

/// `POST /v1/guilds/{id}/edit` — set a guild's name and/or base-camp level
/// (spliced into the guild's `GroupSaveDataMap` RawData).
async fn guild_edit(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Json(body): Json<GuildEditBody>,
) -> Response {
    if let Some(resp) = write_guard(&state) {
        return resp;
    }
    let Ok(gid) = Uuid::parse_str(&id) else {
        return not_found();
    };
    match state.app.bundle().await {
        Ok(b) if b.world.guilds.iter().any(|g| g.id == id) => {}
        Ok(_) => return not_found(),
        Err(e) => return internal_error(e),
    }

    let path = state.app.save_dir().join("Level.sav");
    let name = body.guild_name.clone();
    let level = body.base_camp_level;
    let receipt = match run_edit(&state, move || {
        psm_save::save::edit::edit_sav_file(&path, |gvas| {
            psm_save::save::edit::ops::edit_guild(gvas, gid, name.as_deref(), level)
        })
    })
    .await
    {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    write_ok(&receipt, None)
}

#[derive(serde::Deserialize)]
struct BaseEditBody {
    #[serde(default)]
    area_range: Option<f64>,
    #[serde(default)]
    name: Option<String>,
}

/// `POST /v1/bases/{id}/edit` — set a base camp's build-area radius (`area_range`)
/// and/or name (spliced into the base's `BaseCampSaveData` RawData).
async fn base_edit(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Json(body): Json<BaseEditBody>,
) -> Response {
    if let Some(resp) = write_guard(&state) {
        return resp;
    }
    let Ok(bid) = Uuid::parse_str(&id) else {
        return not_found();
    };
    match state.app.bundle().await {
        Ok(b) if b.world.guilds.iter().flat_map(|g| &g.bases).any(|base| base.id == id) => {}
        Ok(_) => return not_found(),
        Err(e) => return internal_error(e),
    }

    let path = state.app.save_dir().join("Level.sav");
    let area = body.area_range.map(|a| a as f32);
    let name = body.name.clone();
    let receipt = match run_edit(&state, move || {
        psm_save::save::edit::edit_sav_file(&path, |gvas| {
            psm_save::save::edit::ops::edit_base(gvas, bid, area, name.as_deref())
        })
    })
    .await
    {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    write_ok(&receipt, None)
}

#[derive(serde::Deserialize)]
struct ResizeBody {
    slot_num: u32,
}

/// `POST /v1/containers/{id}/resize` — change a container's slot count
/// (upstream PR #299 semantics; shrinking deletes out-of-range slots).
async fn container_resize(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Json(body): Json<ResizeBody>,
) -> Response {
    if let Some(resp) = write_guard(&state) {
        return resp;
    }
    let Ok(cid) = Uuid::parse_str(&id) else {
        return not_found();
    };
    if body.slot_num > 9999 {
        return unprocessable("slot_num out of range (0..=9999)");
    }
    match state.app.bundle().await {
        Ok(b) if b.item_containers.contains_key(&cid) => {}
        Ok(_) => return not_found(),
        Err(e) => return internal_error(e),
    }

    let path = state.app.save_dir().join("Level.sav");
    let n = body.slot_num;
    let receipt = match run_edit(&state, move || {
        psm_save::save::edit::edit_sav_file(&path, |gvas| {
            psm_save::save::edit::ops::resize_container(gvas, cid, n)
        })
    })
    .await
    {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let container = refreshed_container(&state, cid).await;
    write_ok(&receipt, container)
}

#[derive(serde::Deserialize)]
struct SlotBody {
    slot_index: i32,
    static_id: String,
    count: i32,
}

/// `POST /v1/containers/{id}/slot` — set or clear one slot (static items;
/// `static_id: "None"` or `count: 0` clears).
async fn container_slot(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Json(body): Json<SlotBody>,
) -> Response {
    if let Some(resp) = write_guard(&state) {
        return resp;
    }
    let Ok(cid) = Uuid::parse_str(&id) else {
        return not_found();
    };
    if body.count < 0 || body.count > 9999 {
        return unprocessable("count out of range (0..=9999)");
    }
    if body.static_id.len() > 128 {
        return unprocessable("static_id too long");
    }
    match state.app.bundle().await {
        Ok(b) if b.item_containers.contains_key(&cid) => {}
        Ok(_) => return not_found(),
        Err(e) => return internal_error(e),
    }

    let path = state.app.save_dir().join("Level.sav");
    let SlotBody {
        slot_index,
        static_id,
        count,
    } = body;
    let receipt = match run_edit(&state, move || {
        psm_save::save::edit::edit_sav_file(&path, |gvas| {
            psm_save::save::edit::ops::set_container_slot(gvas, cid, slot_index, &static_id, count)
        })
    })
    .await
    {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let container = refreshed_container(&state, cid).await;
    write_ok(&receipt, container)
}

/// `POST /v1/containers/{id}/clear` — remove every occupied slot in one
/// write (one backup, instead of one per stack).
async fn container_clear(
    State(state): State<ServerState>,
    Path(id): Path<String>,
) -> Response {
    if let Some(resp) = write_guard(&state) {
        return resp;
    }
    let Ok(cid) = Uuid::parse_str(&id) else {
        return not_found();
    };
    match state.app.bundle().await {
        Ok(b) if b.item_containers.contains_key(&cid) => {}
        Ok(_) => return not_found(),
        Err(e) => return internal_error(e),
    }

    let path = state.app.save_dir().join("Level.sav");
    let receipt = match run_edit(&state, move || {
        psm_save::save::edit::edit_sav_file(&path, |gvas| {
            psm_save::save::edit::ops::clear_container(gvas, cid)
        })
    })
    .await
    {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let container = refreshed_container(&state, cid).await;
    write_ok(&receipt, container)
}

#[derive(serde::Deserialize, Default)]
struct PlayerEditBody {
    level: Option<u8>,
    exp: Option<i64>,
    status_points: Option<BTreeMap<String, i32>>,
    ext_status_points: Option<BTreeMap<String, i32>>,
}

/// `POST /v1/players/{id}/edit` — level/exp/status-point edits in `Level.sav`.
async fn player_edit(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Json(body): Json<PlayerEditBody>,
) -> Response {
    if let Some(resp) = write_guard(&state) {
        return resp;
    }
    let Some(uid_str) = canonical_uid(&id) else {
        return not_found();
    };
    match state.app.bundle().await {
        Ok(b) if b.world.players.iter().any(|p| p.uid == uid_str) => {}
        Ok(_) => return not_found(),
        Err(e) => return internal_error(e),
    }
    if let Some(resp) = validate_character_edit(body.level, body.exp, &None) {
        return resp;
    }
    if let Some(resp) = validate_points(&body.status_points).or(validate_points(&body.ext_status_points)) {
        return resp;
    }

    let uid = Uuid::parse_str(&uid_str).expect("canonical uid parses");
    let edits = psm_save::save::edit::ops::CharacterEdits {
        level: body.level,
        exp: body.exp,
        status_points: body.status_points,
        ext_status_points: body.ext_status_points,
        ..Default::default()
    };
    let path = state.app.save_dir().join("Level.sav");
    match run_edit(&state, move || {
        psm_save::save::edit::edit_sav_file(&path, |gvas| {
            psm_save::save::edit::ops::edit_character(
                gvas,
                psm_save::save::edit::ops::CharTarget::Player(uid),
                &edits,
            )
        })
    })
    .await
    {
        Ok(receipt) => write_ok(&receipt, None),
        Err(resp) => resp,
    }
}

#[derive(serde::Deserialize, Default)]
struct PalEditBody {
    level: Option<u8>,
    exp: Option<i64>,
    nickname: Option<String>,
    passive_skills: Option<Vec<String>>,
    active_skills: Option<Vec<String>>,
    learned_skills: Option<Vec<String>>,
    rank: Option<u8>,
    rank_hp: Option<u8>,
    rank_attack: Option<u8>,
    rank_defense: Option<u8>,
    rank_craftspeed: Option<u8>,
    talent_hp: Option<u8>,
    talent_shot: Option<u8>,
    talent_defense: Option<u8>,
    /// `"Male"` / `"Female"` (bare or `EPalGenderType::`-prefixed).
    gender: Option<String>,
    /// `EPalWorkSuitability::…` code → rank (0..=5).
    work_suitability: Option<BTreeMap<String, i32>>,
}

/// `POST /v1/pals/{id}/edit` — pal edits in `Level.sav`, keyed by pal
/// instance id.
async fn pal_edit(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Json(body): Json<PalEditBody>,
) -> Response {
    if let Some(resp) = write_guard(&state) {
        return resp;
    }
    let Some(iid_str) = canonical_uid(&id) else {
        return not_found();
    };
    match state.app.bundle().await {
        Ok(b) if b.world.pals.iter().any(|p| p.instance_id == iid_str) => {}
        Ok(_) => return not_found(),
        Err(e) => return internal_error(e),
    }
    if let Some(resp) = validate_character_edit(body.level, body.exp, &body.nickname) {
        return resp;
    }
    // On-disk condenser Rank is 1-based (1 = uncondensed, 5 = 4 stars);
    // 0 is not a value the game writes.
    if body.rank.is_some_and(|r| !(1..=5).contains(&r)) {
        return unprocessable("rank out of range (1..=5; 1 = no condenser stars)");
    }
    if let Some(g) = &body.gender {
        let bare = g.strip_prefix("EPalGenderType::").unwrap_or(g);
        if bare != "Male" && bare != "Female" {
            return unprocessable("gender must be Male or Female");
        }
    }
    if let Some(ws) = &body.work_suitability {
        if let Some(resp) = validate_work_suitability(ws) {
            return resp;
        }
    }
    for l in [&body.passive_skills, &body.active_skills, &body.learned_skills]
        .into_iter()
        .flatten()
    {
        if l.len() > 64 || l.iter().any(|s| s.len() > 128) {
            return unprocessable("skill list too long");
        }
    }

    let iid = Uuid::parse_str(&iid_str).expect("canonical uid parses");
    let edits = psm_save::save::edit::ops::CharacterEdits {
        level: body.level,
        exp: body.exp,
        nickname: body.nickname,
        passive_skills: body.passive_skills,
        active_skills: body.active_skills,
        learned_skills: body.learned_skills,
        rank: body.rank,
        rank_hp: body.rank_hp,
        rank_attack: body.rank_attack,
        rank_defense: body.rank_defense,
        rank_craftspeed: body.rank_craftspeed,
        talent_hp: body.talent_hp,
        talent_shot: body.talent_shot,
        talent_defense: body.talent_defense,
        gender: body.gender,
        work_suitability: body.work_suitability,
        ..Default::default()
    };
    let path = state.app.save_dir().join("Level.sav");
    match run_edit(&state, move || {
        psm_save::save::edit::edit_sav_file(&path, |gvas| {
            psm_save::save::edit::ops::edit_character(
                gvas,
                psm_save::save::edit::ops::CharTarget::Instance(iid),
                &edits,
            )
        })
    })
    .await
    {
        Ok(receipt) => write_ok(&receipt, None),
        Err(resp) => resp,
    }
}

/// Process-wide species-stats catalog (compiled in, parsed once).
static PAL_STATS: OnceLock<psm_save::save::reference::PalStatsCatalog> = OnceLock::new();

fn pal_stats() -> &'static psm_save::save::reference::PalStatsCatalog {
    PAL_STATS.get_or_init(psm_save::save::reference::load_pal_stats)
}

/// Find a pal in the bundle by canonical instance-id string, or 404.
async fn find_pal(state: &ServerState, id: &str) -> Result<(Uuid, Pal), Response> {
    let Some(iid_str) = canonical_uid(id) else {
        return Err(not_found());
    };
    let bundle = state.app.bundle().await.map_err(internal_error)?;
    let pal = bundle
        .world
        .pals
        .iter()
        .find(|p| p.instance_id == iid_str)
        .cloned()
        .ok_or_else(not_found)?;
    Ok((Uuid::parse_str(&iid_str).expect("canonical uid parses"), pal))
}

/// `POST /v1/pals/{id}/heal` — full restore: revive/sick state cleared,
/// sanity 100, stomach to species max, HP to the computed maximum (ports
/// palworld-save-pal `pal.heal` + `hp = max_hp`).
async fn pal_heal(State(state): State<ServerState>, Path(id): Path<String>) -> Response {
    if let Some(resp) = write_guard(&state) {
        return resp;
    }
    let (iid, pal) = match find_pal(&state, &id).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let stats = pal_stats().for_character_id(&pal.character_id);
    let heal = psm_save::save::edit::ops::HealValues {
        hp: stats.map(|s| {
            psm_save::save::reference::max_hp(
                s,
                pal.level,
                pal.talent_hp,
                pal.rank,
                pal.rank_hp,
                pal.is_boss || pal.is_lucky,
            )
        }),
        stomach: stats.map(|s| s.stomach as f32).filter(|s| *s > 0.0).unwrap_or(150.0),
        sanity: 100.0,
    };

    let path = state.app.save_dir().join("Level.sav");
    match run_edit(&state, move || {
        psm_save::save::edit::edit_sav_file(&path, |gvas| {
            psm_save::save::edit::ops::heal_pal(gvas, iid, &heal)
        })
    })
    .await
    {
        Ok(receipt) => write_ok(&receipt, None),
        Err(resp) => resp,
    }
}

/// Resolve a base (by id, across all guilds) to the full [`Pal`]s stationed at
/// it (its worker container's pals, back-filled in `Base.pals`). 404 if the base
/// id is invalid or not found.
async fn base_pals(state: &ServerState, base_id: &str) -> Result<Vec<Pal>, Response> {
    let Ok(bid) = Uuid::parse_str(base_id) else {
        return Err(not_found());
    };
    let bundle = state.app.bundle().await.map_err(internal_error)?;
    let base = bundle
        .world
        .guilds
        .iter()
        .flat_map(|g| &g.bases)
        .find(|b| Uuid::parse_str(&b.id).is_ok_and(|x| x == bid))
        .ok_or_else(not_found)?;
    let ids: std::collections::HashSet<&str> = base.pals.iter().map(String::as_str).collect();
    Ok(bundle
        .world
        .pals
        .iter()
        .filter(|p| ids.contains(p.instance_id.as_str()))
        .cloned()
        .collect())
}

/// `POST /v1/bases/{id}/pals/heal` — fully restore every pal stationed at the
/// base in one write (revive, clear WorkerSick etc., sanity 100, stomach + HP
/// to each pal's species max). No-op-safe if the base has no pals.
async fn base_pals_heal(State(state): State<ServerState>, Path(id): Path<String>) -> Response {
    if let Some(resp) = write_guard(&state) {
        return resp;
    }
    let pals = match base_pals(&state, &id).await {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let heals: Vec<(Uuid, psm_save::save::edit::ops::HealValues)> = pals
        .iter()
        .filter_map(|pal| {
            let iid = Uuid::parse_str(&pal.instance_id).ok()?;
            let stats = pal_stats().for_character_id(&pal.character_id);
            let heal = psm_save::save::edit::ops::HealValues {
                hp: stats.map(|s| {
                    psm_save::save::reference::max_hp(
                        s,
                        pal.level,
                        pal.talent_hp,
                        pal.rank,
                        pal.rank_hp,
                        pal.is_boss || pal.is_lucky,
                    )
                }),
                stomach: stats.map(|s| s.stomach as f32).filter(|s| *s > 0.0).unwrap_or(150.0),
                sanity: 100.0,
            };
            Some((iid, heal))
        })
        .collect();
    if heals.is_empty() {
        return unprocessable("this base has no pals to heal");
    }

    let path = state.app.save_dir().join("Level.sav");
    match run_edit(&state, move || {
        psm_save::save::edit::edit_sav_file(&path, |gvas| {
            psm_save::save::edit::ops::batch_heal(gvas, &heals)
        })
    })
    .await
    {
        Ok(receipt) => write_ok(&receipt, None),
        Err(resp) => resp,
    }
}

#[derive(serde::Deserialize)]
struct BasePalsEditBody {
    #[serde(default)]
    level: Option<u8>,
    #[serde(default)]
    exp: Option<i64>,
    /// Work-suitability (`work affinity`) ranks to set on every base pal, e.g.
    /// `{ Handcraft: 5, Mining: 5, ... }`. Ranks are 0..=5 (our app convention).
    #[serde(default)]
    work_suitability: Option<std::collections::BTreeMap<String, i32>>,
}

/// `POST /v1/bases/{id}/pals/edit` — apply the same edits (level/EXP and/or work
/// suitability) to every pal stationed at the base in one write. Used for "level
/// all" and "max work affinity" on a base.
async fn base_pals_edit(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Json(body): Json<BasePalsEditBody>,
) -> Response {
    if let Some(resp) = write_guard(&state) {
        return resp;
    }
    if body.level.is_some_and(|l| !(1..=100).contains(&l)) {
        return unprocessable("level out of range (1..=100)");
    }
    if body.exp.is_some_and(|e| e < 0) {
        return unprocessable("exp must be non-negative");
    }
    if let Some(ws) = &body.work_suitability {
        if let Some(resp) = validate_work_suitability(ws) {
            return resp;
        }
    }
    if body.level.is_none() && body.exp.is_none() && body.work_suitability.is_none() {
        return unprocessable("no pal edits requested");
    }

    let pals = match base_pals(&state, &id).await {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let targets: Vec<Uuid> = pals
        .iter()
        .filter_map(|p| Uuid::parse_str(&p.instance_id).ok())
        .collect();
    if targets.is_empty() {
        return unprocessable("this base has no pals to edit");
    }

    let edits = psm_save::save::edit::ops::CharacterEdits {
        level: body.level,
        exp: body.exp,
        work_suitability: body.work_suitability,
        ..Default::default()
    };
    let path = state.app.save_dir().join("Level.sav");
    match run_edit(&state, move || {
        psm_save::save::edit::edit_sav_file(&path, |gvas| {
            psm_save::save::edit::ops::batch_edit_characters(gvas, &targets, &edits)
        })
    })
    .await
    {
        Ok(receipt) => write_ok(&receipt, None),
        Err(resp) => resp,
    }
}

/// `POST /v1/pals/{id}/delete` — remove the pal and its container slots.
async fn pal_delete(State(state): State<ServerState>, Path(id): Path<String>) -> Response {
    if let Some(resp) = write_guard(&state) {
        return resp;
    }
    let (iid, _pal) = match find_pal(&state, &id).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let path = state.app.save_dir().join("Level.sav");
    match run_edit(&state, move || {
        psm_save::save::edit::edit_sav_file(&path, |gvas| {
            psm_save::save::edit::ops::delete_pal(gvas, iid)
        })
    })
    .await
    {
        Ok(receipt) => write_ok(&receipt, None),
        Err(resp) => resp,
    }
}

#[derive(Serialize)]
struct CloneResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    backup: Option<String>,
    /// The new pal's instance id.
    instance_id: String,
}

/// `POST /v1/pals/{id}/clone` — duplicate the pal into its owner's pal box.
async fn pal_clone(State(state): State<ServerState>, Path(id): Path<String>) -> Response {
    if let Some(resp) = write_guard(&state) {
        return resp;
    }
    let (iid, pal) = match find_pal(&state, &id).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if pal.owner_uid.is_empty() {
        return unprocessable("pal has no owner; only owned pals can be cloned");
    }
    // Target: the owner's pal box, from their per-player save.
    let save = match state.app.player_save(&pal.owner_uid).await {
        Ok(s) => s,
        Err(e) => return internal_error(e),
    };
    let Ok(target) = Uuid::parse_str(&save.containers.pal_storage) else {
        return unprocessable("owner's pal box container id is missing");
    };
    let new_iid = Uuid::new_v4();

    let path = state.app.save_dir().join("Level.sav");
    match run_edit(&state, move || {
        psm_save::save::edit::edit_sav_file(&path, |gvas| {
            psm_save::save::edit::ops::clone_pal(gvas, iid, target, new_iid)
        })
    })
    .await
    {
        Ok(receipt) => Json(CloneResponse {
            ok: true,
            backup: receipt.backup.as_ref().map(|b| b.display().to_string()),
            instance_id: new_iid.to_string(),
        })
        .into_response(),
        Err(resp) => resp,
    }
}

#[derive(serde::Deserialize, Default)]
struct TechBody {
    #[serde(default)]
    unlock: Vec<String>,
    #[serde(default)]
    relock: Vec<String>,
    technology_point: Option<i32>,
    boss_technology_point: Option<i32>,
}

/// `POST /v1/players/{id}/technologies` — unlock/relock technologies and set
/// technology points in the per-player `<UID>.sav`.
async fn player_technologies(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Json(body): Json<TechBody>,
) -> Response {
    if let Some(resp) = write_guard(&state) {
        return resp;
    }
    let Some(uid_str) = canonical_uid(&id) else {
        return not_found();
    };
    if body.unlock.len() > 1000
        || body.relock.len() > 1000
        || body.unlock.iter().chain(&body.relock).any(|s| s.len() > 128)
    {
        return unprocessable("technology list too long");
    }
    if body.technology_point.is_some_and(|v| !(0..=100_000).contains(&v))
        || body.boss_technology_point.is_some_and(|v| !(0..=100_000).contains(&v))
    {
        return unprocessable("technology points out of range");
    }

    let sav = state
        .app
        .save_dir()
        .join("Players")
        .join(format!("{}.sav", uid_str.replace('-', "").to_uppercase()));
    if !sav.is_file() {
        return not_found();
    }

    let edits = psm_save::save::edit::ops::TechEdits {
        unlock: body.unlock,
        relock: body.relock,
        technology_point: body.technology_point,
        boss_technology_point: body.boss_technology_point,
    };
    match run_edit(&state, move || {
        psm_save::save::edit::edit_sav_file(&sav, |gvas| {
            psm_save::save::edit::ops::edit_player_technologies(gvas, &edits)
        })
    })
    .await
    {
        Ok(receipt) => write_ok(&receipt, None),
        Err(resp) => resp,
    }
}

#[derive(serde::Deserialize)]
struct PlayerMapBody {
    /// Unlock every fast-travel point (SaveData.RecordData.FastTravelPointUnlockFlag).
    #[serde(default)]
    unlock_all_fast_travel: bool,
}

/// `POST /v1/players/{id}/map` — per-player map/progression unlocks written to
/// the per-player `<UID>.sav`. Currently: unlock all fast-travel points.
///
/// (Map *reveal* is intentionally not offered here: the explored-map fog lives
/// in each client's local `LocalData.sav`, which is not part of a dedicated
/// server's save set, so the bridge cannot reach it.)
async fn player_map(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Json(body): Json<PlayerMapBody>,
) -> Response {
    if let Some(resp) = write_guard(&state) {
        return resp;
    }
    let Some(uid_str) = canonical_uid(&id) else {
        return not_found();
    };
    if !body.unlock_all_fast_travel {
        return unprocessable("no map unlock requested");
    }

    let sav = state
        .app
        .save_dir()
        .join("Players")
        .join(format!("{}.sav", uid_str.replace('-', "").to_uppercase()));
    if !sav.is_file() {
        return not_found();
    }

    match run_edit(&state, move || {
        psm_save::save::edit::edit_sav_file(&sav, |gvas| {
            psm_save::save::edit::ops::unlock_all_fast_travel(gvas)
        })
    })
    .await
    {
        Ok(receipt) => write_ok(&receipt, None),
        Err(resp) => resp,
    }
}

/// Shared 422 validation for level/exp/nickname edits; `Some(response)` on
/// the first violation.
fn validate_character_edit(
    level: Option<u8>,
    exp: Option<i64>,
    nickname: &Option<String>,
) -> Option<Response> {
    if level.is_some_and(|l| !(1..=100).contains(&l)) {
        return Some(unprocessable("level out of range (1..=100)"));
    }
    if exp.is_some_and(|e| e < 0) {
        return Some(unprocessable("exp must be non-negative"));
    }
    if nickname.as_ref().is_some_and(|n| n.chars().count() > 64) {
        return Some(unprocessable("nickname too long (max 64 chars)"));
    }
    None
}

/// 422 for absurd status-point values.
fn validate_points(points: &Option<BTreeMap<String, i32>>) -> Option<Response> {
    if let Some(p) = points {
        if p.len() > 64 || p.values().any(|v| !(0..=100_000).contains(v)) {
            return Some(unprocessable("status points out of range"));
        }
    }
    None
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

/// Border validation for a `work_suitability` edit map, shared by the single-pal
/// and base-batch edit handlers. Returns `Some(422)` on any problem.
///
/// Keys must be the fully-qualified on-disk enum values
/// (`EPalWorkSuitability::Handcraft`, not a bare `Handcraft`): a bare suffix
/// never matches an existing entry and would append a malformed enum the game
/// can't resolve. The write layer rejects bare codes too, but catching it here
/// gives clients a clean 422 instead of a deep save error.
fn validate_work_suitability(ws: &BTreeMap<String, i32>) -> Option<Response> {
    if ws.len() > 32 || ws.keys().any(|k| k.len() > 64) {
        return Some(unprocessable("work suitability map too large"));
    }
    if ws.values().any(|v| !(0..=5).contains(v)) {
        return Some(unprocessable("work suitability ranks out of range (0..=5)"));
    }
    if let Some(k) = ws.keys().find(|k| !k.starts_with("EPalWorkSuitability::")) {
        return Some(unprocessable(format!(
            "work suitability key must be fully qualified (EPalWorkSuitability::…): {k}"
        )));
    }
    None
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
