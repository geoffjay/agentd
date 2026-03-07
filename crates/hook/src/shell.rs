//! Shell integration script generation.
//!
//! Generates shell functions that users add to their shell configuration files
//! (`~/.zshrc`, `~/.bashrc`, etc.) to enable automatic event capture.
//!
//! The generated scripts use preexec/precmd hooks to capture command start/end
//! times and forward completed events to the hook service via HTTP.

/// Shell type for integration script generation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shell {
    Zsh,
    Bash,
    Fish,
}

impl Shell {
    /// Parse a shell name string into a [`Shell`] variant.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "zsh" => Some(Shell::Zsh),
            "bash" => Some(Shell::Bash),
            "fish" => Some(Shell::Fish),
            _ => None,
        }
    }
}

/// Generate a shell integration script for the given shell and hook service URL.
///
/// The generated script defines hooks that:
/// 1. Record the command start time (preexec)
/// 2. On command completion, POST a JSON event to the hook service
///
/// # Arguments
///
/// - `shell` - Target shell type
/// - `hook_url` - Base URL of the hook service (e.g. `http://localhost:17002`)
///
/// # Examples
///
/// ```
/// use hook::shell::{Shell, generate_integration};
///
/// let script = generate_integration(Shell::Zsh, "http://localhost:17002");
/// assert!(script.contains("agentd_hook_url"));
/// assert!(script.contains("preexec"));
/// ```
pub fn generate_integration(shell: Shell, hook_url: &str) -> String {
    match shell {
        Shell::Zsh => generate_zsh(hook_url),
        Shell::Bash => generate_bash(hook_url),
        Shell::Fish => generate_fish(hook_url),
    }
}

fn generate_zsh(hook_url: &str) -> String {
    format!(
        r#"# agentd-hook zsh integration
# Add this to your ~/.zshrc:
#   source <(agentd-hook shell zsh)
# Or copy the output manually.

agentd_hook_url="{hook_url}"
_agentd_hook_start_time=0
_agentd_hook_last_command=""

agentd_hook_preexec() {{
  _agentd_hook_start_time=$EPOCHREALTIME
  _agentd_hook_last_command="$1"
}}

agentd_hook_precmd() {{
  local exit_code=$?
  if [[ -z "$_agentd_hook_last_command" ]]; then
    return
  fi

  local end_time=$EPOCHREALTIME
  local duration_ms=$(( (end_time - _agentd_hook_start_time) * 1000 ))
  duration_ms=${{duration_ms%.*}}

  local payload
  payload=$(printf '{{"kind":"shell","command":"%s","exit_code":%d,"duration_ms":%d,"metadata":{{"shell":"zsh"}}}}' \
    "$(echo "$_agentd_hook_last_command" | sed 's/"/\\"/g')" \
    "$exit_code" \
    "$duration_ms")

  curl -sf -X POST \
    -H "Content-Type: application/json" \
    -d "$payload" \
    "$agentd_hook_url/events" &>/dev/null &
  disown

  _agentd_hook_last_command=""
}}

autoload -Uz add-zsh-hook
add-zsh-hook preexec agentd_hook_preexec
add-zsh-hook precmd agentd_hook_precmd
"#
    )
}

fn generate_bash(hook_url: &str) -> String {
    format!(
        r#"# agentd-hook bash integration
# Add this to your ~/.bashrc:
#   source <(agentd-hook shell bash)
# Or copy the output manually.

agentd_hook_url="{hook_url}"
_agentd_hook_start_time=0
_agentd_hook_last_command=""

agentd_hook_preexec() {{
  _agentd_hook_start_time=$SECONDS
  _agentd_hook_last_command="$BASH_COMMAND"
}}

agentd_hook_precmd() {{
  local exit_code=$?
  if [[ -z "$_agentd_hook_last_command" ]]; then
    return
  fi

  local duration_ms=$(( (SECONDS - _agentd_hook_start_time) * 1000 ))

  local payload
  payload=$(printf '{{"kind":"shell","command":"%s","exit_code":%d,"duration_ms":%d,"metadata":{{"shell":"bash"}}}}' \
    "$(echo "$_agentd_hook_last_command" | sed 's/"/\\"/g')" \
    "$exit_code" \
    "$duration_ms")

  curl -sf -X POST \
    -H "Content-Type: application/json" \
    -d "$payload" \
    "$agentd_hook_url/events" >/dev/null 2>&1 &

  _agentd_hook_last_command=""
}}

trap 'agentd_hook_preexec' DEBUG
PROMPT_COMMAND="${{PROMPT_COMMAND:+$PROMPT_COMMAND; }}agentd_hook_precmd"
"#
    )
}

fn generate_fish(hook_url: &str) -> String {
    format!(
        r#"# agentd-hook fish integration
# Add this to your ~/.config/fish/config.fish:
#   agentd-hook shell fish | source
# Or copy the output manually.

set -g agentd_hook_url "{hook_url}"
set -g _agentd_hook_start_time 0
set -g _agentd_hook_last_command ""

function agentd_hook_preexec --on-event fish_preexec
  set -g _agentd_hook_start_time (date +%s%3N)
  set -g _agentd_hook_last_command $argv[1]
end

function agentd_hook_postexec --on-event fish_postexec
  set exit_code $status
  if test -z "$_agentd_hook_last_command"
    return
  end

  set end_time (date +%s%3N)
  set duration_ms (math $end_time - $_agentd_hook_start_time)

  set payload (printf '{{"kind":"shell","command":"%s","exit_code":%d,"duration_ms":%d,"metadata":{{"shell":"fish"}}}}' \
    (echo "$_agentd_hook_last_command" | string replace -a '"' '\\"') \
    $exit_code \
    $duration_ms)

  curl -sf -X POST \
    -H "Content-Type: application/json" \
    -d $payload \
    $agentd_hook_url/events &>/dev/null &

  set -g _agentd_hook_last_command ""
end
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_from_str() {
        assert_eq!(Shell::from_str("zsh"), Some(Shell::Zsh));
        assert_eq!(Shell::from_str("bash"), Some(Shell::Bash));
        assert_eq!(Shell::from_str("fish"), Some(Shell::Fish));
        assert_eq!(Shell::from_str("Zsh"), Some(Shell::Zsh));
        assert_eq!(Shell::from_str("unknown"), None);
    }

    #[test]
    fn test_zsh_integration_contains_hook_url() {
        let script = generate_integration(Shell::Zsh, "http://localhost:17002");
        assert!(script.contains("http://localhost:17002"));
        assert!(script.contains("agentd_hook_preexec"));
        assert!(script.contains("agentd_hook_precmd"));
        assert!(script.contains("agentd_hook_url"));
    }

    #[test]
    fn test_bash_integration_contains_hook_url() {
        let script = generate_integration(Shell::Bash, "http://localhost:17002");
        assert!(script.contains("http://localhost:17002"));
        assert!(script.contains("agentd_hook_preexec"));
        assert!(script.contains("PROMPT_COMMAND"));
    }

    #[test]
    fn test_fish_integration_contains_hook_url() {
        let script = generate_integration(Shell::Fish, "http://localhost:17002");
        assert!(script.contains("http://localhost:17002"));
        assert!(script.contains("fish_preexec"));
        assert!(script.contains("fish_postexec"));
    }

    #[test]
    fn test_zsh_script_references_exit_code() {
        let script = generate_integration(Shell::Zsh, "http://localhost:17002");
        assert!(script.contains("exit_code"));
    }

    #[test]
    fn test_bash_script_references_exit_code() {
        let script = generate_integration(Shell::Bash, "http://localhost:17002");
        assert!(script.contains("exit_code"));
    }
}
