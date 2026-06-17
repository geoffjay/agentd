//! HTTP reverse proxy for the core service API gateway.
//!
//! [`ProxyConfig`] maps service names to their upstream base URLs.
//! [`proxy_request`] forwards an incoming Axum request to a downstream service,
//! injecting `X-Tenant-ID` and `X-Request-ID` headers.
//!
//! # Streaming
//!
//! Request and response bodies are streamed — the proxy does not buffer large
//! payloads in memory.
//!
//! # Configuration
//!
//! Each upstream URL is taken from the shared `[services.core]` config section
//! (see [`agentd_common::config::CoreConfig`]) via [`ProxyConfig::from_config`].
//! For backward compatibility, a matching bare environment variable still
//! overrides the configured value when set:
//!
//! | Service       | Config key (`[services.core]`) | Env override        | Default                    |
//! |---------------|--------------------------------|---------------------|----------------------------|
//! | orchestrator  | `orchestrator_url`             | `ORCHESTRATOR_URL`  | `http://localhost:17006`   |
//! | notify        | `notify_url`                   | `NOTIFY_URL`        | `http://localhost:17004`   |
//! | ask           | `ask_url`                      | `ASK_URL`           | `http://localhost:17001`   |
//! | wrap          | `wrap_url`                     | `WRAP_URL`          | `http://localhost:17005`   |
//! | hook          | `hook_url`                     | `HOOK_URL`          | `http://localhost:17002`   |
//! | monitor       | `monitor_url`                  | `MONITOR_URL`       | `http://localhost:17003`   |
//! | memory        | `memory_url`                   | `MEMORY_URL`        | `http://localhost:17008`   |
//! | communicate   | `communicate_url`              | `COMMUNICATE_URL`   | `http://localhost:17010`   |
//! | index         | `index_url`                    | `INDEX_URL`         | `http://localhost:17012`   |
//! | knowledge     | `knowledge_url`                | `KNOWLEDGE_URL`     | `http://localhost:17011`   |

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
    /// Build a [`ProxyConfig`] from the shared `[services.core]` configuration.
    ///
    /// Each upstream URL is taken from the corresponding `core.*_url` config
    /// value (which itself layers config-file values over compiled defaults).
    /// For backward compatibility, a matching bare `*_URL` environment variable
    /// — `ORCHESTRATOR_URL`, `NOTIFY_URL`, etc. — still overrides the config
    /// value when set.
    pub fn from_config(core: &agentd_common::config::CoreConfig) -> Self {
        let services = [
            ("orchestrator", "ORCHESTRATOR_URL", core.orchestrator_url.as_str()),
            ("notify", "NOTIFY_URL", core.notify_url.as_str()),
            ("ask", "ASK_URL", core.ask_url.as_str()),
            ("wrap", "WRAP_URL", core.wrap_url.as_str()),
            ("hook", "HOOK_URL", core.hook_url.as_str()),
            ("monitor", "MONITOR_URL", core.monitor_url.as_str()),
            ("memory", "MEMORY_URL", core.memory_url.as_str()),
            ("communicate", "COMMUNICATE_URL", core.communicate_url.as_str()),
            ("index", "INDEX_URL", core.index_url.as_str()),
            ("knowledge", "KNOWLEDGE_URL", core.knowledge_url.as_str()),
        ]
        .into_iter()
        .map(|(name, env, configured)| {
            let url = std::env::var(env).unwrap_or_else(|_| configured.to_string());
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

    /// Build a [`ProxyConfig`] from compiled defaults overlaid with bare
    /// `*_URL` environment variables.
    ///
    /// Equivalent to [`from_config`](Self::from_config) with the default
    /// [`agentd_common::config::CoreConfig`]; used in tests and when no shared
    /// config is available.
    pub fn from_env() -> Self {
        Self::from_config(&agentd_common::config::CoreConfig::default())
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
    use std::sync::Mutex;

    /// Serialises tests that mutate the process-global `ORCHESTRATOR_URL` env
    /// var so they don't race when the harness runs them concurrently.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_proxy_config_defaults() {
        let cfg = ProxyConfig::from_env();
        assert!(cfg.url_for("orchestrator").is_some());
        assert!(cfg.url_for("notify").is_some());
        assert!(cfg.url_for("ask").is_some());
        assert!(cfg.url_for("wrap").is_some());
        assert!(cfg.url_for("hook").is_some());
        assert!(cfg.url_for("monitor").is_some());
        assert!(cfg.url_for("memory").is_some());
        assert!(cfg.url_for("communicate").is_some());
        assert!(cfg.url_for("index").is_some());
        assert!(cfg.url_for("knowledge").is_some());
        assert!(cfg.url_for("nonexistent").is_none());
    }

    #[test]
    fn test_proxy_config_new_service_defaults() {
        let cfg = ProxyConfig::from_env();
        assert_eq!(cfg.url_for("memory"), Some("http://localhost:17008"));
        assert_eq!(cfg.url_for("communicate"), Some("http://localhost:17010"));
        assert_eq!(cfg.url_for("index"), Some("http://localhost:17012"));
        assert_eq!(cfg.url_for("knowledge"), Some("http://localhost:17011"));
    }

    #[test]
    fn test_proxy_config_env_override() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var("ORCHESTRATOR_URL", "http://custom-host:9999");
        let cfg = ProxyConfig::from_env();
        assert_eq!(cfg.url_for("orchestrator"), Some("http://custom-host:9999"));
        std::env::remove_var("ORCHESTRATOR_URL");
    }

    #[test]
    fn test_proxy_config_from_config() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Guard against an ambient ORCHESTRATOR_URL leaking from another test.
        std::env::remove_var("ORCHESTRATOR_URL");
        let core = agentd_common::config::CoreConfig {
            orchestrator_url: "http://localhost:7006".to_string(),
            memory_url: "http://localhost:7008".to_string(),
            ..Default::default()
        };

        let cfg = ProxyConfig::from_config(&core);

        assert_eq!(cfg.url_for("orchestrator"), Some("http://localhost:7006"));
        assert_eq!(cfg.url_for("memory"), Some("http://localhost:7008"));
        // Untouched services keep the configured defaults.
        assert_eq!(cfg.url_for("notify"), Some("http://localhost:17004"));
    }

    #[test]
    fn test_proxy_config_env_overrides_config() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var("ORCHESTRATOR_URL", "http://from-env:9999");
        let core = agentd_common::config::CoreConfig {
            orchestrator_url: "http://from-config:7006".to_string(),
            ..Default::default()
        };

        let cfg = ProxyConfig::from_config(&core);
        std::env::remove_var("ORCHESTRATOR_URL");

        assert_eq!(cfg.url_for("orchestrator"), Some("http://from-env:9999"));
    }
}
