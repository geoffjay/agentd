//! Write install-time service configuration to agentd/config.toml.
//!
//! Rather than encoding ports, backend type, and inter-service URLs as
//! environment variables in generated plist / systemd unit files (where they
//! permanently override the user's config), the installer writes these values
//! directly into the agentd config file using a gap-filling merge: existing
//! user values are preserved and only missing keys are populated.

use crate::platform::ServiceInfo;
use anyhow::{Context, Result};
use colored::Colorize;

/// Write install-managed configuration values into `agentd/config.toml`.
///
/// Uses a gap-filling merge — keys already present in the user's config file
/// are left untouched.  Only absent keys receive the install defaults, so
/// user customisations (e.g. a different backend or port) survive a reinstall.
pub fn write_install_config(services: &[ServiceInfo]) -> Result<()> {
    let config_path = agentd_common::config::config_file_path()
        .ok_or_else(|| anyhow::anyhow!("Could not resolve agentd config file path"))?;

    // Load existing config as a raw TOML value so we preserve sections we
    // don't own (e.g. [linear], [mcp], user-set service overrides).
    let mut existing: toml::Value = if config_path.exists() {
        let s = std::fs::read_to_string(&config_path)
            .with_context(|| format!("reading {}", config_path.display()))?;
        toml::from_str(&s).unwrap_or_else(|_| toml::Value::Table(Default::default()))
    } else {
        toml::Value::Table(Default::default())
    };

    let defaults = build_install_defaults(services);
    merge_toml_defaults(&mut existing, defaults);

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating config directory {}", parent.display()))?;
    }

    std::fs::write(&config_path, toml::to_string_pretty(&existing)?)
        .with_context(|| format!("writing {}", config_path.display()))?;

    println!("  {} agentd/config.toml updated ({})", "✓".green(), config_path.display());
    Ok(())
}

/// Build the TOML value containing all install-managed defaults.
fn build_install_defaults(services: &[ServiceInfo]) -> toml::Value {
    use toml::map::Map;
    use toml::Value;

    let orchestrator_port = services
        .iter()
        .find(|s| s.name == "orchestrator")
        .map(|s| s.port)
        .unwrap_or(7006);

    let communicate_port = services
        .iter()
        .find(|s| s.name == "communicate")
        .map(|s| s.port)
        .unwrap_or(7010);

    let mut services_map = Map::new();

    for svc in services {
        let mut svc_table = Map::new();
        svc_table.insert("port".to_string(), Value::Integer(svc.port as i64));
        services_map.insert(svc.name.to_string(), Value::Table(svc_table));
    }

    // orchestrator: backend + communicate_url + subprocess_path
    if let Some(Value::Table(t)) = services_map.get_mut("orchestrator") {
        t.insert("backend".to_string(), Value::String("subprocess".to_string()));
        t.insert(
            "communicate_url".to_string(),
            Value::String(format!("http://localhost:{communicate_port}")),
        );
        // Capture the installer's PATH so the orchestrator can inject it into
        // spawned subprocesses at runtime (LaunchAgent/systemd units start with
        // a bare system PATH that lacks tools like `claude`).
        let path = std::env::var("PATH").unwrap_or_default();
        t.insert("subprocess_path".to_string(), Value::String(path));
    }

    // wrap: backend
    if let Some(Value::Table(t)) = services_map.get_mut("wrap") {
        t.insert("backend".to_string(), Value::String("subprocess".to_string()));
    }

    // ask: orchestrator_url (compiled default points at dev port 17006)
    if let Some(Value::Table(t)) = services_map.get_mut("ask") {
        t.insert(
            "orchestrator_url".to_string(),
            Value::String(format!("http://localhost:{orchestrator_port}")),
        );
    }

    let mut root = Map::new();
    root.insert("services".to_string(), Value::Table(services_map));
    Value::Table(root)
}

/// Recursively gap-fill `base` with values from `defaults`.
///
/// For tables: recurse so nested keys are filled individually.
/// For scalar values: `base` wins — only insert if the key is absent.
fn merge_toml_defaults(base: &mut toml::Value, defaults: toml::Value) {
    if let (toml::Value::Table(base_t), toml::Value::Table(def_t)) = (base, defaults) {
        for (k, def_v) in def_t {
            let entry = base_t.entry(k).or_insert_with(|| def_v.clone());
            // If both sides are tables, recurse so inner keys are gap-filled
            // rather than replacing the whole sub-table.
            if matches!(entry, toml::Value::Table(_)) {
                merge_toml_defaults(entry, def_v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ServiceInfo;

    fn test_services() -> Vec<ServiceInfo> {
        vec![
            ServiceInfo { name: "ask", binary: "agentd-ask", port: 7001, port_env: "AGENTD_ASK_PORT" },
            ServiceInfo { name: "notify", binary: "agentd-notify", port: 7004, port_env: "AGENTD_NOTIFY_PORT" },
            ServiceInfo { name: "wrap", binary: "agentd-wrap", port: 7005, port_env: "AGENTD_WRAP_PORT" },
            ServiceInfo {
                name: "orchestrator",
                binary: "agentd-orchestrator",
                port: 7006,
                port_env: "AGENTD_ORCHESTRATOR_PORT",
            },
            ServiceInfo {
                name: "communicate",
                binary: "agentd-communicate",
                port: 7010,
                port_env: "AGENTD_COMMUNICATE_PORT",
            },
        ]
    }

    #[test]
    fn test_build_defaults_sets_ports() {
        let svcs = test_services();
        let defaults = build_install_defaults(&svcs);
        let services = defaults["services"].as_table().unwrap();
        assert_eq!(services["ask"]["port"].as_integer(), Some(7001));
        assert_eq!(services["wrap"]["port"].as_integer(), Some(7005));
        assert_eq!(services["orchestrator"]["port"].as_integer(), Some(7006));
    }

    #[test]
    fn test_build_defaults_sets_backends() {
        let svcs = test_services();
        let defaults = build_install_defaults(&svcs);
        let services = defaults["services"].as_table().unwrap();
        assert_eq!(services["orchestrator"]["backend"].as_str(), Some("subprocess"));
        assert_eq!(services["wrap"]["backend"].as_str(), Some("subprocess"));
        // ask should not have a backend field
        assert!(services["ask"].get("backend").is_none());
    }

    #[test]
    fn test_build_defaults_sets_communicate_url() {
        let svcs = test_services();
        let defaults = build_install_defaults(&svcs);
        let services = defaults["services"].as_table().unwrap();
        assert_eq!(
            services["orchestrator"]["communicate_url"].as_str(),
            Some("http://localhost:7010")
        );
    }

    #[test]
    fn test_build_defaults_sets_ask_orchestrator_url() {
        let svcs = test_services();
        let defaults = build_install_defaults(&svcs);
        let services = defaults["services"].as_table().unwrap();
        assert_eq!(
            services["ask"]["orchestrator_url"].as_str(),
            Some("http://localhost:7006")
        );
    }

    #[test]
    fn test_merge_fills_gaps() {
        let mut base: toml::Value = toml::from_str(r#"
            [services.wrap]
            backend = "docker"
        "#)
        .unwrap();

        let defaults: toml::Value = toml::from_str(r#"
            [services.wrap]
            port = 7005
            backend = "subprocess"

            [services.orchestrator]
            port = 7006
            backend = "subprocess"
        "#)
        .unwrap();

        merge_toml_defaults(&mut base, defaults);

        let svcs = base["services"].as_table().unwrap();
        // user value preserved
        assert_eq!(svcs["wrap"]["backend"].as_str(), Some("docker"));
        // gap filled
        assert_eq!(svcs["wrap"]["port"].as_integer(), Some(7005));
        // new section inserted
        assert_eq!(svcs["orchestrator"]["port"].as_integer(), Some(7006));
        assert_eq!(svcs["orchestrator"]["backend"].as_str(), Some("subprocess"));
    }

    #[test]
    fn test_merge_preserves_unrelated_sections() {
        let mut base: toml::Value = toml::from_str(r#"
            [linear]
            api_key = "lin_api_secret"
        "#)
        .unwrap();

        let defaults: toml::Value =
            toml::from_str("[services.wrap]\nport = 7005\n").unwrap();

        merge_toml_defaults(&mut base, defaults);

        assert_eq!(base["linear"]["api_key"].as_str(), Some("lin_api_secret"));
        assert_eq!(base["services"]["wrap"]["port"].as_integer(), Some(7005));
    }
}
