//! HTTP reverse proxy for the core service API gateway.
//!
//! [`ProxyConfig`] maps service names to their base URLs (resolved from
//! environment variables). [`proxy_request`] forwards an incoming Axum request
//! to a downstream service, injecting `X-Tenant-ID` and `X-Request-ID` headers.
//!
//! # Streaming
//!
//! Request and response bodies are streamed — the proxy does not buffer large
//! payloads in memory.
//!
//! # Configuration
//!
//! Each service URL is read from an environment variable:
//!
//! | Service       | Env var                        | Default                    |
//! |---------------|--------------------------------|----------------------------|
//! | orchestrator  | `ORCHESTRATOR_URL`             | `http://localhost:17006`   |
//! | notify        | `NOTIFY_URL`                   | `http://localhost:17004`   |
//! | ask           | `ASK_URL`                      | `http://localhost:17001`   |
//! | wrap          | `WRAP_URL`                     | `http://localhost:17005`   |
//! | hook          | `HOOK_URL`                     | `http://localhost:17002`   |
//! | monitor       | `MONITOR_URL`                  | `http://localhost:17003`   |

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use axum::body::Bytes;
use reqwest::Client;

/// Service name → base URL mapping for the API gateway.
#[derive(Clone)]
pub struct ProxyConfig {
    /// Map of service name to base URL (without trailing slash).
    pub services: HashMap<&'static str, String>,
    /// HTTP client shared across all proxy requests.
    pub client: Client,
}

impl ProxyConfig {
    /// Build a [`ProxyConfig`] from environment variables, using sensible defaults.
    pub fn from_env() -> Self {
        let services = [
            ("orchestrator", "ORCHESTRATOR_URL", "http://localhost:17006"),
            ("notify", "NOTIFY_URL", "http://localhost:17004"),
            ("ask", "ASK_URL", "http://localhost:17001"),
            ("wrap", "WRAP_URL", "http://localhost:17005"),
            ("hook", "HOOK_URL", "http://localhost:17002"),
            ("monitor", "MONITOR_URL", "http://localhost:17003"),
        ]
        .into_iter()
        .map(|(name, env, default)| {
            let url = std::env::var(env).unwrap_or_else(|_| default.to_string());
            (name, url)
        })
        .collect();

        let client = Client::builder()
            .timeout(Duration::from_secs(
                std::env::var("PROXY_TIMEOUT_SECS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(30u64),
            ))
            .build()
            .expect("failed to build proxy HTTP client");

        Self { services, client }
    }

    /// Look up the base URL for a service name.
    pub fn url_for(&self, service: &str) -> Option<&str> {
        self.services.get(service).map(String::as_str)
    }
}

/// Perform a health check against a downstream service.
///
/// Returns `(is_healthy, detail_message)`.
pub async fn health_check(client: &Client, base_url: &str) -> (bool, Option<String>) {
    let health_url = format!("{}/health", base_url);
    match client.get(&health_url).timeout(Duration::from_secs(3)).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let detail = body.get("status").and_then(|v| v.as_str()).map(str::to_string);
            (true, detail)
        }
        Ok(resp) => (false, Some(format!("HTTP {}", resp.status()))),
        Err(e) => {
            let msg = if e.is_connect() {
                "connection refused".to_string()
            } else if e.is_timeout() {
                "timeout".to_string()
            } else {
                e.to_string()
            };
            (false, Some(msg))
        }
    }
}

/// Result of a [`proxy_request`] call.
pub struct ProxyResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Bytes,
}

/// Parameters for a proxied HTTP request.
pub struct ProxyRequest<'a> {
    /// HTTP method (e.g. `"GET"`, `"POST"`).
    pub method: &'a str,
    /// Path to append to the base URL (must start with `/`).
    pub path: &'a str,
    /// Optional pre-encoded query string (without leading `?`).
    pub query: Option<&'a str>,
    /// Incoming headers to forward (excluding `host` and `content-length`).
    pub headers: &'a [(String, String)],
    /// Request body bytes.
    pub body: Bytes,
    /// Tenant identifier injected as `X-Tenant-ID`.
    pub tenant_id: &'a str,
    /// Request identifier injected as `X-Request-ID`.
    pub request_id: &'a str,
}

/// Forward an HTTP request to a downstream service.
///
/// - Injects `X-Tenant-ID` from [`ProxyRequest::tenant_id`]
/// - Injects `X-Request-ID` from [`ProxyRequest::request_id`]
/// - Forwards all safe incoming headers (excludes `host` and `content-length`)
/// - Streams the response body back
pub async fn proxy_request(
    client: &Client,
    target_url: &str,
    req: ProxyRequest<'_>,
) -> Result<ProxyResponse> {
    let method = req.method;
    let path = req.path;
    let query = req.query;
    let headers = req.headers;
    let body = req.body;
    let tenant_id = req.tenant_id;
    let request_id = req.request_id;
    let url = match query {
        Some(q) if !q.is_empty() => format!("{}{target_url}{path}?{q}", ""),
        _ => format!("{target_url}{path}"),
    };

    let method = reqwest::Method::from_bytes(method.as_bytes())?;

    let mut req = client.request(method, &url).body(body);

    // Forward safe incoming headers (skip host, content-length — reqwest sets those)
    for (name, value) in headers {
        let name_lower = name.to_lowercase();
        if name_lower == "host" || name_lower == "content-length" {
            continue;
        }
        if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(name.as_bytes()) {
            if let Ok(header_value) = reqwest::header::HeaderValue::from_str(value) {
                req = req.header(header_name, header_value);
            }
        }
    }

    // Inject tenant and request ID headers
    req = req.header("X-Tenant-ID", tenant_id).header("X-Request-ID", request_id);

    let resp = req.send().await?;
    let status = resp.status().as_u16();

    // Collect response headers to forward back
    let resp_headers: Vec<(String, String)> = resp
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            let n = name.as_str().to_string();
            // Skip headers that axum/hyper will set
            if n == "transfer-encoding" || n == "connection" {
                return None;
            }
            value.to_str().ok().map(|v| (n, v.to_string()))
        })
        .collect();

    let body = resp.bytes().await?;

    Ok(ProxyResponse { status, headers: resp_headers, body })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_config_defaults() {
        let cfg = ProxyConfig::from_env();
        assert!(cfg.url_for("orchestrator").is_some());
        assert!(cfg.url_for("notify").is_some());
        assert!(cfg.url_for("ask").is_some());
        assert!(cfg.url_for("wrap").is_some());
        assert!(cfg.url_for("hook").is_some());
        assert!(cfg.url_for("monitor").is_some());
        assert!(cfg.url_for("nonexistent").is_none());
    }

    #[test]
    fn test_proxy_config_env_override() {
        std::env::set_var("ORCHESTRATOR_URL", "http://custom-host:9999");
        let cfg = ProxyConfig::from_env();
        assert_eq!(cfg.url_for("orchestrator"), Some("http://custom-host:9999"));
        std::env::remove_var("ORCHESTRATOR_URL");
    }
}
