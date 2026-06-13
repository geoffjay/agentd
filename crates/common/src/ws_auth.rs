//! WebSocket token validation helper.
//!
//! Downstream WS handlers call `validate_ws_token` at upgrade time to
//! authenticate the caller. The helper calls `GET {core_url}/auth/me`
//! with the bearer token and returns the (user_id, org_id) on success,
//! or an error on 401/network failure.
//!
//! This is a best-effort check: if `AGENTD_CORE_SERVICE_URL` is not set
//! the function returns `None` so services work without the gateway in
//! dev/test.

use anyhow::{Context, Result};

/// Result of a successful token validation.
#[derive(Debug, Clone)]
pub struct WsAuthInfo {
    pub user_id: String,
    pub organization_id: Option<String>,
}

/// Validate a bearer token by calling the core service's `/auth/me` endpoint.
///
/// Returns `None` if `AGENTD_CORE_SERVICE_URL` is not set (dev mode).
/// Returns `Some(Err(...))` if validation fails (wrong token, network error).
/// Returns `Some(Ok(WsAuthInfo))` on success.
pub async fn validate_ws_token(token: &str) -> Option<Result<WsAuthInfo>> {
    let core_url = std::env::var("AGENTD_CORE_SERVICE_URL").ok()?;

    let url = format!("{}/auth/me", core_url);
    let client = reqwest::Client::new();

    let result = async {
        let resp = client
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .context("failed to reach core service for WS auth")?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(anyhow::anyhow!("unauthorized"));
        }

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("core auth check returned {}", resp.status()));
        }

        let body: serde_json::Value =
            resp.json().await.context("failed to parse auth/me response")?;

        Ok(WsAuthInfo {
            user_id: body["id"].as_str().unwrap_or("unknown").to_string(),
            organization_id: body["active_organization_id"].as_str().map(|s| s.to_string()),
        })
    }
    .await;

    Some(result)
}
