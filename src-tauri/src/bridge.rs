//! HTTP client for the PSM Bridge companion (`psm-bridge.exe`).
//!
//! Mirrors [`crate::api`], but targets the bridge's Tier-2 REST surface
//! (`http://<host>:<port>/v1`) with a Bearer token instead of the Palworld
//! REST API's HTTP Basic auth. The token lives in this Rust layer and never
//! reaches the webview — the same posture as the admin password.

use serde_json::Value;

#[derive(Clone)]
pub struct BridgeCreds {
    pub host: String,
    pub port: u16,
    pub token: String,
}

fn base(c: &BridgeCreds) -> String {
    format!("http://{}:{}/v1", c.host, c.port)
}

fn map_err(e: reqwest::Error) -> String {
    if e.is_connect() {
        "Could not reach the bridge. Is psm-bridge.exe running on the server?".into()
    } else if e.is_timeout() {
        "The bridge took too long to respond.".into()
    } else {
        e.to_string()
    }
}

/// GET a bridge endpoint (e.g. `/health`, `/players`, `/players/{uid}`) and
/// return the parsed JSON body. `path` must start with `/`.
pub async fn get(c: &BridgeCreds, path: &str) -> Result<Value, String> {
    if !path.starts_with('/') || path.contains("..") {
        return Err("Invalid bridge path.".into());
    }
    let url = format!("{}{}", base(c), path);
    let resp = reqwest::Client::new()
        .get(url)
        .bearer_auth(&c.token)
        .header("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(map_err)?;
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("Bridge authentication failed. Check the bridge token.".into());
    }
    if !resp.status().is_success() {
        return Err(format!("Bridge returned {}", resp.status()));
    }
    resp.json::<Value>().await.map_err(map_err)
}

/// POST a bridge endpoint (e.g. `/server/start`) with an optional JSON body
/// and return the parsed JSON. On an error status, surfaces the bridge's own
/// `detail` message when present (e.g. "the server is already running").
pub async fn post(c: &BridgeCreds, path: &str, body: Option<Value>) -> Result<Value, String> {
    if !path.starts_with('/') || path.contains("..") {
        return Err("Invalid bridge path.".into());
    }
    let url = format!("{}{}", base(c), path);
    let mut req = reqwest::Client::new()
        .post(url)
        .bearer_auth(&c.token)
        .header("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(60));
    if let Some(body) = body {
        req = req.json(&body);
    }
    let resp = req.send().await.map_err(map_err)?;
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("Bridge authentication failed. Check the bridge token.".into());
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if let Some(detail) = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v.get("detail").and_then(|d| d.as_str()).map(str::to_string))
        {
            return Err(detail);
        }
        return Err(format!("Bridge returned {status}"));
    }
    resp.json::<Value>().await.map_err(map_err)
}
