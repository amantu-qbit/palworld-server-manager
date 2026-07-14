use crate::api::{self, Creds};
use crate::bridge::{self, BridgeCreds};
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::State;

#[derive(Default)]
pub struct AppState {
    pub creds: Mutex<Option<Creds>>,
    /// Tier-2 bridge credentials (host + chosen port + Bearer token). `None`
    /// until the owner configures the bridge; absence means Tier 1 only.
    pub bridge: Mutex<Option<BridgeCreds>>,
}

fn current(state: &State<AppState>) -> Result<Creds, String> {
    state
        .creds
        .lock()
        .map_err(|_| "state error".to_string())?
        .clone()
        .ok_or_else(|| "Not connected.".to_string())
}

fn current_bridge(state: &State<AppState>) -> Result<BridgeCreds, String> {
    state
        .bridge
        .lock()
        .map_err(|_| "state error".to_string())?
        .clone()
        .ok_or_else(|| "Bridge not configured.".to_string())
}

#[tauri::command]
pub async fn test_connection(host: String, port: u16, password: String) -> Result<(), String> {
    api::get(&Creds { host, port, password }, "/info")
        .await
        .map(|_| ())
}

#[tauri::command]
pub async fn get_info(state: State<'_, AppState>) -> Result<Value, String> {
    api::get(&current(&state)?, "/info").await
}

#[tauri::command]
pub async fn get_metrics(state: State<'_, AppState>) -> Result<Value, String> {
    api::get(&current(&state)?, "/metrics").await
}

#[tauri::command]
pub async fn get_players(state: State<'_, AppState>) -> Result<Value, String> {
    api::get(&current(&state)?, "/players").await
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Value, String> {
    api::get(&current(&state)?, "/settings").await
}

#[tauri::command]
pub async fn get_game_data(state: State<'_, AppState>) -> Result<Value, String> {
    api::get(&current(&state)?, "/game-data").await
}

#[tauri::command]
pub async fn announce(state: State<'_, AppState>, message: String) -> Result<(), String> {
    api::post(&current(&state)?, "/announce", json!({ "message": message })).await
}

#[tauri::command]
pub async fn kick(
    state: State<'_, AppState>,
    userid: String,
    message: String,
) -> Result<(), String> {
    api::post(
        &current(&state)?,
        "/kick",
        json!({ "userid": userid, "message": message }),
    )
    .await
}

#[tauri::command]
pub async fn ban(
    state: State<'_, AppState>,
    userid: String,
    message: String,
) -> Result<(), String> {
    api::post(
        &current(&state)?,
        "/ban",
        json!({ "userid": userid, "message": message }),
    )
    .await
}

#[tauri::command]
pub async fn unban(state: State<'_, AppState>, userid: String) -> Result<(), String> {
    api::post(&current(&state)?, "/unban", json!({ "userid": userid })).await
}

#[tauri::command]
pub async fn save_world(state: State<'_, AppState>) -> Result<(), String> {
    api::post(&current(&state)?, "/save", json!({})).await
}

#[tauri::command]
pub async fn shutdown(
    state: State<'_, AppState>,
    waittime: u32,
    message: String,
) -> Result<(), String> {
    api::post(
        &current(&state)?,
        "/shutdown",
        json!({ "waittime": waittime, "message": message }),
    )
    .await
}

#[tauri::command]
pub async fn force_stop(state: State<'_, AppState>) -> Result<(), String> {
    api::post(&current(&state)?, "/stop", json!({})).await
}

#[tauri::command]
pub async fn save_connection(
    state: State<'_, AppState>,
    host: String,
    port: u16,
    password: String,
) -> Result<(), String> {
    *state.creds.lock().map_err(|_| "state error".to_string())? =
        Some(Creds { host, port, password });
    Ok(())
}

#[tauri::command]
pub async fn load_connection(state: State<'_, AppState>) -> Result<Option<Value>, String> {
    Ok(state
        .creds
        .lock()
        .map_err(|_| "state error".to_string())?
        .as_ref()
        .map(|c| json!({ "host": c.host, "port": c.port })))
}

// --- Tier-2 bridge (psm-bridge.exe) -----------------------------------------

/// Store the bridge credentials so subsequent `bridge_get` calls are
/// authenticated. The token stays in the Rust layer, out of the webview.
#[tauri::command]
pub async fn save_bridge(
    state: State<'_, AppState>,
    host: String,
    port: u16,
    token: String,
) -> Result<(), String> {
    *state.bridge.lock().map_err(|_| "state error".to_string())? =
        Some(BridgeCreds { host, port, token });
    Ok(())
}

/// Forget any stored bridge credentials (e.g. on disconnect or when the owner
/// clears the bridge fields).
#[tauri::command]
pub async fn clear_bridge(state: State<'_, AppState>) -> Result<(), String> {
    *state.bridge.lock().map_err(|_| "state error".to_string())? = None;
    Ok(())
}

/// GET a bridge endpoint (`/health`, `/players`, `/players/{uid}`, …) and
/// return the parsed JSON. Errors as a readable string on auth/transport
/// failure — the frontend surfaces it.
#[tauri::command]
pub async fn bridge_get(state: State<'_, AppState>, path: String) -> Result<Value, String> {
    bridge::get(&current_bridge(&state)?, &path).await
}
