//! Runtime configuration served to the SPA at `GET /config.json`.
//!
//! The prebuilt frontend cannot know per-host service locations at build
//! time, so it fetches this document at startup. For each browser-facing
//! service the document carries the configured port; the SPA combines it
//! with the page's own protocol and hostname unless an explicit `url`
//! override is present (`[services.ui.public_urls]` in `config.toml`).

use serde::Serialize;
use std::collections::BTreeMap;

/// Document returned by `GET /config.json`.
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeConfig {
    /// Version of the agentd-ui service that produced the document.
    pub version: &'static str,
    /// Browser-facing services keyed by short service name.
    pub services: BTreeMap<&'static str, ServiceEntry>,
}

/// Location of a single service as exposed to the browser.
#[derive(Debug, Clone, Serialize)]
pub struct ServiceEntry {
    /// Listen port of the service.
    pub port: u16,
    /// Explicit browser-facing URL override; when present it takes
    /// precedence over host + port derivation in the SPA.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Services the SPA talks to directly. `hook`, `wrap`, and `core` are
/// backend-only and intentionally not exposed.
const BROWSER_SERVICES: &[&str] =
    &["ask", "notify", "orchestrator", "memory", "monitor", "communicate"];

/// Build the runtime configuration from the shared agentd config.
pub fn build(shared: &agentd_common::config::AgentdConfig) -> RuntimeConfig {
    let s = &shared.services;
    let overrides = &s.ui.public_urls;

    let mut services = BTreeMap::new();
    for &name in BROWSER_SERVICES {
        let port = match name {
            "ask" => s.ask.port,
            "notify" => s.notify.port,
            "orchestrator" => s.orchestrator.port,
            "memory" => s.memory.port,
            "monitor" => s.monitor.port,
            "communicate" => s.communicate.port,
            _ => unreachable!("unknown browser service {name}"),
        };
        services.insert(name, ServiceEntry { port, url: overrides.get(name).cloned() });
    }

    RuntimeConfig { version: env!("CARGO_PKG_VERSION"), services }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentd_common::config::AgentdConfig;

    #[test]
    fn test_build_default_ports() {
        let cfg = AgentdConfig::default();
        let runtime = build(&cfg);

        assert_eq!(runtime.services.len(), BROWSER_SERVICES.len());
        assert_eq!(runtime.services["ask"].port, 17001);
        assert_eq!(runtime.services["orchestrator"].port, 17006);
        assert_eq!(runtime.services["communicate"].port, 17010);
        assert!(runtime.services.values().all(|s| s.url.is_none()));
    }

    #[test]
    fn test_build_with_public_url_override() {
        let mut cfg = AgentdConfig::default();
        cfg.services
            .ui
            .public_urls
            .insert("orchestrator".to_string(), "https://agentd.example.com/orch".to_string());
        let runtime = build(&cfg);

        assert_eq!(
            runtime.services["orchestrator"].url.as_deref(),
            Some("https://agentd.example.com/orch")
        );
        assert!(runtime.services["ask"].url.is_none());
    }

    #[test]
    fn test_serialization_shape() {
        let cfg = AgentdConfig::default();
        let json = serde_json::to_value(build(&cfg)).unwrap();

        assert!(json["version"].is_string());
        assert_eq!(json["services"]["notify"]["port"], 17004);
        // `url` is omitted entirely when not overridden.
        assert!(json["services"]["notify"].get("url").is_none());
    }
}
