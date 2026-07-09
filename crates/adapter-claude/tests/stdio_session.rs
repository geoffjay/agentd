//! End-to-end stdio session against a fake `claude` binary.
//!
//! Drives the real adapter binary over the mandatory stdio AAP binding with a
//! scripted fake claude, exercising the handshake, a prompt turn, and a tool
//! approval round trip — the conformance path from `docs/spec/agent-protocol-v1.md`.

use std::io::Write as _;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// A fake `claude` that speaks just enough `stream-json` for the test:
/// on the first `user` message it emits an assistant text block, a tool_use,
/// a can_use_tool control_request, then a result once approval arrives.
const FAKE_CLAUDE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
while IFS= read -r line; do
  case "$line" in
    *'"type":"user"'*)
      printf '%s\n' '{"type":"assistant","message":{"content":[{"type":"text","text":"working"},{"type":"tool_use","id":"tu1","name":"Bash","input":{"command":"ls"}}]}}'
      printf '%s\n' '{"type":"control_request","request_id":"req1","request":{"subtype":"can_use_tool","tool_name":"Bash","input":{"command":"ls"}}}'
      ;;
    *'"behavior":"allow"'*)
      printf '%s\n' '{"type":"result","is_error":false,"result":"done","usage":{"input_tokens":5,"output_tokens":2},"total_cost_usd":0.001,"num_turns":1,"duration_ms":10,"duration_api_ms":8}'
      ;;
  esac
done
"#;

#[tokio::test]
async fn full_stdio_turn_with_approval() {
    // Write the fake claude to a temp script.
    let dir = std::env::temp_dir().join(format!("aap-adapter-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fake = dir.join("claude");
    {
        let mut f = std::fs::File::create(&fake).unwrap();
        f.write_all(FAKE_CLAUDE.as_bytes()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            f.set_permissions(std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    let mut child = Command::new(env!("CARGO_BIN_EXE_agentd-adapter-claude"))
        .env("AGENTD_AAP_TRANSPORT", "stdio")
        .env("AGENTD_CLAUDE_BIN", &fake)
        .env("RUST_LOG", "error")
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();

    async fn send(stdin: &mut tokio::process::ChildStdin, s: String) {
        stdin.write_all(s.as_bytes()).await.unwrap();
        stdin.write_all(b"\n").await.unwrap();
        stdin.flush().await.unwrap();
    }

    let result = tokio::time::timeout(Duration::from_secs(20), async {
        // Handshake.
        send(
            &mut stdin,
            r#"{"type":"initialize","protocol_version":1,"workspace":{"cwd":"."}}"#.to_string(),
        )
        .await;

        let mut saw_ready = false;
        let mut saw_message = false;
        let mut saw_tool_call = false;
        let mut saw_turn_complete = false;
        let mut approved = false;

        while let Ok(Some(line)) = lines.next_line().await {
            let v: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            match v["type"].as_str().unwrap_or("") {
                "ready" => {
                    saw_ready = true;
                    // Now start the turn.
                    send(
                        &mut stdin,
                        r#"{"type":"prompt","turn_id":"t1","content":"do it"}"#.to_string(),
                    )
                    .await;
                }
                "message" => {
                    saw_message = true;
                    assert_eq!(v["turn_id"], "t1");
                }
                "tool_call" => {
                    saw_tool_call = true;
                    assert_eq!(v["name"], "Bash");
                }
                "approval_request" => {
                    // Approve, echoing the input back as updated_input.
                    let rid = v["request_id"].as_str().unwrap().to_string();
                    approved = true;
                    send(
                        &mut stdin,
                        format!(
                            r#"{{"type":"approval_response","request_id":"{rid}","decision":"allow","updated_input":{{"command":"ls"}}}}"#
                        ),
                    )
                    .await;
                }
                "turn_complete" => {
                    saw_turn_complete = true;
                    assert_eq!(v["turn_id"], "t1");
                    assert_eq!(v["usage"]["input_tokens"], 5);
                    break;
                }
                _ => {}
            }
        }

        (saw_ready, saw_message, saw_tool_call, approved, saw_turn_complete)
    })
    .await
    .expect("adapter session timed out");

    let _ = child.start_kill();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(result.0, "expected ready");
    assert!(result.1, "expected message");
    assert!(result.2, "expected tool_call");
    assert!(result.3, "expected approval_request");
    assert!(result.4, "expected turn_complete");
}
