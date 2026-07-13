//! HTTP server: axum router, Bearer-auth middleware, and route handlers.
//!
//! Every route is wrapped in a Bearer-auth check — there is no unauthenticated
//! route in this API, including `/v1/health`. Phase 1b is read-only:
//! `writes_enabled` is hard-coded `false` regardless of configuration.
//!
//! SECURITY: all endpoint routes are registered in [`app_routes`], the one
//! function `.layer()` in [`router`] wraps. See that function's doc comment
//! before adding a new route.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::state::AppState;

/// Combined router state: the decoded-save cache plus the configured auth
/// token. Cheap to clone — both fields are `Arc`s.
#[derive(Clone)]
struct ServerState {
    app: Arc<AppState>,
    token: Arc<String>,
}

/// SECURITY: EVERY endpoint route MUST be added inside this function.
///
/// `router()` applies the Bearer-auth layer to exactly the routes registered
/// here. A route registered anywhere else (e.g. merged into the `Router`
/// returned by `router()` after `.layer()` has already been applied) is
/// UNAUTHENTICATED — axum's `.layer()` only wraps routes added to the
/// `Router` *before* the `.layer()` call, not routes added after.
fn app_routes() -> Router<ServerState> {
    Router::new().route("/v1/health", get(health))
    // Task 6+ endpoints go HERE, all auto-protected.
}

/// Build the bridge HTTP router.
///
/// Currently exposes `GET /v1/health`. Every route registered in
/// [`app_routes`] is wrapped in a Bearer-auth layer (see [`auth`]) that
/// requires the `Authorization` header to equal `Bearer {token}`.
pub fn router(state: Arc<AppState>, token: Arc<String>) -> Router {
    let server_state = ServerState { app: state, token };

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
