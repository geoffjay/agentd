//! Resolution of an agent's `agent_type` to an AAP adapter program.
//!
//! agentd speaks the vendor-neutral agentd Agent Protocol (AAP) to every agent
//! through an adapter process. This module maps a configured `agent_type` (e.g.
//! `"claude"`) to the adapter binary that fronts it (e.g.
//! `agentd-adapter-claude`).
//!
//! Resolution order:
//! 1. An explicit override in the environment variable
//!    `AGENTD_ADAPTER_<TYPE>` (TYPE upper-cased, non-alphanumerics → `_`),
//!    whose value is the adapter program path.
//! 2. The conventional binary name `agentd-adapter-<type>`, resolved as a
//!    sibling of the running orchestrator binary when present (app-bundle safe;
//!    a bare name can resolve to the wrong sibling), otherwise left as a bare
//!    name for `PATH` resolution.

/// Return the sibling of the current executable with the given file name, if it
/// exists as a regular file.
fn sibling_of_current_exe(name: &str) -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let candidate = dir.join(name);
    if candidate.is_file() {
        Some(candidate.to_string_lossy().into_owned())
    } else {
        None
    }
}

/// The environment-variable override key for an agent type.
fn override_env_key(agent_type: &str) -> String {
    let sanitized: String = agent_type
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' })
        .collect();
    format!("AGENTD_ADAPTER_{sanitized}")
}

/// Resolve the adapter program path for an `agent_type`.
///
/// This never fails: if resolution falls through to the bare binary name, a
/// missing binary surfaces later as a spawn failure (the agent goes `Failed`).
pub fn resolve_adapter_program(agent_type: &str) -> String {
    if let Ok(path) = std::env::var(override_env_key(agent_type)) {
        let path = path.trim();
        if !path.is_empty() {
            return path.to_string();
        }
    }

    let binary = format!("agentd-adapter-{agent_type}");
    sibling_of_current_exe(&binary).unwrap_or(binary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_key_sanitizes() {
        assert_eq!(override_env_key("claude"), "AGENTD_ADAPTER_CLAUDE");
        assert_eq!(override_env_key("my-agent.v2"), "AGENTD_ADAPTER_MY_AGENT_V2");
    }

    #[test]
    fn default_binary_name_for_unknown_type() {
        // With no override and no sibling, falls back to the bare conventional name.
        let prog = resolve_adapter_program("gemini");
        assert!(prog.ends_with("agentd-adapter-gemini"));
    }
}
