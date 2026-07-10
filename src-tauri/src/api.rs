use serde_json::Value;

#[derive(Clone)]
pub struct Creds {
    pub host: String,
    pub port: u16,
    pub password: String,
}

fn base(c: &Creds) -> String {
    format!("http://{}:{}/v1/api", c.host, c.port)
}

fn map_err(e: reqwest::Error) -> String {
    if e.is_connect() {
        "Could not reach the server. Is it running and is the REST API enabled?".into()
    } else if e.is_timeout() {
        "The server took too long to respond.".into()
    } else {
        e.to_string()
    }
}

pub async fn get(c: &Creds, path: &str) -> Result<Value, String> {
    let url = format!("{}{}", base(c), path);
    let resp = reqwest::Client::new()
        .get(url)
        .basic_auth("admin", Some(&c.password))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(map_err)?;
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("Authentication failed. Check the admin password.".into());
    }
    if !resp.status().is_success() {
        return Err(format!("Server returned {}", resp.status()));
    }
    resp.json::<Value>().await.map_err(map_err)
}

pub async fn post(c: &Creds, path: &str, body: Value) -> Result<(), String> {
    let url = format!("{}{}", base(c), path);
    let resp = reqwest::Client::new()
        .post(url)
        .basic_auth("admin", Some(&c.password))
        .json(&body)
        .send()
        .await
        .map_err(map_err)?;
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("Authentication failed. Check the admin password.".into());
    }
    if !resp.status().is_success() {
        return Err(format!("Server returned {}", resp.status()));
    }
    Ok(())
}
