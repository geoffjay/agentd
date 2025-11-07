# CRUSH.md – Agentd Development Guide

## Overview
Agentd is a modular daemon suite for macOS providing notification, ask, hook, and monitor services with a unified CLI (`agent`).  It is built as a Cargo workspace with separate crates for each component.

## Project Structure
```
agentd/
├─ crates/
│  ├─ ask/          # Ask service daemon
│  ├─ cli/          # `agent` command line interface
│  ├─ hook/         # Hook daemon (future)
│  ├─ monitor/      # System monitor daemon (future)
│  ├─ notify/       # Notification service daemon
│  ├─ ollama/       # Ollama integration library
│  ├─ wrap/         # tmux wrapper utilities
│  ├─ ui/           # GUI (Agent.app) binary
│  ├─ xtask/        # Build & CI helper (install, start‑services, etc.)
│  └─ ...
├─ contrib/
│  └─ scripts/install.sh   # Interactive install script
├─ docs/            # Documentation files
├─ bacon.toml       # Development jobs (check, clippy, test, run…)
└─ Cargo.toml       # Workspace definition
```

## Building & Development Commands
- **Standard build**: `cargo build --release` (builds all crates).
- **Workspace helper** (`xtask`):
  - `cargo xtask install-user` – user‑level install (no sudo). Creates binaries, plist files, and log directory.
  - `cargo xtask start-services` – loads all LaunchAgent plists.
  - `cargo xtask stop-services` – unloads the plists.
  - `cargo xtask restart-services` – restart all services.
- **Bacon jobs** (defined in `bacon.toml`):
  - `bacon check` – `cargo check` for fast compile‑time validation.
  - `bacon clippy` – run Clippy linting.
  - `bacon test` – `cargo test` (default job, shows output).
  - `bacon run` – `cargo run` (starts the GUI or a binary).
  - `bacon run-long` – useful for long‑running daemons.

## Installing & Running Services
1. Run the interactive script (or use `xtask`):
   ```bash
   ./contrib/scripts/install.sh   # will build, copy binaries, install plists
   ```
2. After installation, start services:
   ```bash
   cargo xtask start-services
   ```
3. Verify they are running:
   ```bash
   launchctl list | grep com.geoffjay.agentd-
   ```

## CLI (`agent`) Usage
- Primary entry point: `agent <subcommand> …`
- Sub‑commands are grouped per service: `notify`, `ask`, `wrap` (plus placeholders `hook` & `monitor`).
- Example:
  ```bash
  agent notify create --title "Build Failed" --message "Tests failed" --priority high --requires-response
  agent ask trigger
  ```
- **Environment variables** to override service URLs:
  - `NOTIFY_SERVICE_URL` (default `http://localhost:7004`)
  - `ASK_SERVICE_URL` (default `http://localhost:7001`)
  - `WRAP_SERVICE_URL` (default `http://localhost:7005`)
- When run without arguments, the GUI (`Agent.app`) launches.

## Service Ports
| Service | Default Port | Env Override |
|---------|--------------|--------------|
| Notify  | 3000 (CLI uses 7004 internally) | `NOTIFY_SERVICE_URL` |
| Ask     | 3001 (CLI uses 7001) | `ASK_SERVICE_URL` |
| Wrap    | 7005 | `WRAP_SERVICE_URL` |
| Hook    | – (not implemented) |
| Monitor | – (not implemented) |

## Naming Conventions & Patterns
- Crate names are snake_case (`agentd-notify`, `agentd-ask`).
- Binary names match crate name (e.g., `agentd-notify`).
- CLI binary is `agent` (installed as a symlink `agent` in `$PREFIX/bin`).
- Modules follow Rust conventions: `src/` contains `mod.rs` or `lib.rs`; public API re‑exported in `crate::types`.
- Service clients (`notify::client::NotifyClient`, etc.) live in their crate and expose async `new(url)` constructors.

## Testing
- Unit & integration tests live alongside code (`#[cfg(test)]`).
- Run via Bacon or Cargo:
  ```bash
  bacon test            # shows output, runs all crates
  cargo test -p agentd-notify   # test a single crate
  ```
- Linting with Clippy (`bacon clippy`).

## Gotchas & Tips
- **Log directory permissions** – `install_user` creates `$PREFIX/var/log` and may need sudo to `chown` it.  Check ownership if services cannot write logs.
- **Default service URLs** in the CLI differ from the REST ports shown in the README; the CLI adds a proxy layer (e.g., `localhost:7004` forwards to the notify daemon on `3000`).
- **LaunchAgents** are user‑level; they live in `~/Library/LaunchAgents`.  Use `launchctl unload` to stop a service.
- **Bacon `run-long`** should be used for daemons that never exit; set `background = false`.
- **Adding a new service** – create a new crate under `crates/`, expose a client module, add a subcommand in `cli/src/commands/mod.rs`, and add a plist in `contrib/plists/`.

## Contributing
1. Fork the repo, create a feature branch.
2. Use `cargo fmt` and `cargo clippy` to keep style consistent.
3. Run `bacon test` before pushing.
4. Update this `CRUSH.md` if you modify build flow, CLI flags, or project layout.

---
Generated with Crush.