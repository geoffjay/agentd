ask: watchexec -r -e rs -w crates/ask cargo run -p agentd-ask
notify: watchexec -r -e rs -w crates/notify cargo run -p agentd-notify
orchestrator: watchexec -r -e rs -w crates/orchestrator cargo run -p agentd-orchestrator
hook: watchexec -r -e rs -w crates/hook cargo run -p agentd-hook
monitor: watchexec -r -e rs -w crates/monitor cargo run -p agentd-monitor
rustdoc: watchexec -r -e rs cargo doc --no-deps
docs: zensical serve
ollama: ollama serve
baml: baml serve --from ./baml_src
