# agentd — top-level build automation
#
# Usage:
#   make help              Show available targets
#   make build             Build the Rust workspace
#   make test              Run all tests
#   make docker-build-claude  Build the Claude Code Docker image locally

.PHONY: help build test clippy fmt fmt-fix \
	build-release build-ui \
	install-user install-user-pam install-system install-system-pam \
	uninstall-user uninstall-system \
	docker-build-claude docker-build-claude-multiarch docker-run-claude

# Default image name — matches the DEFAULT_IMAGE constant in crates/wrap/src/docker.rs
CLAUDE_IMAGE ?= agentd-claude:latest

# Set PAM=1 to compile agentd-core with system-user (PAM) login support.
# macOS needs no extra packages; on Linux install the PAM dev headers first
# (libpam0g-dev on Debian/Ubuntu, pam-devel on RHEL/Fedora).
PAM ?= 0

# Freshly-built CLI binary. It is the workspace bin `cli`; the installer renames
# it to `agent` on install. The installed `agent` (on PATH) is used to uninstall.
CLI_BIN := target/release/cli
UI_DIST := ui/dist

help: ## Show this help message
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-24s\033[0m %s\n", $$1, $$2}'

# ── Rust ─────────────────────────────────────────────────────────────

build: ## Build the Rust workspace
	cargo build --workspace

test: ## Run all workspace tests
	cargo test --workspace

clippy: ## Run clippy lints
	cargo clippy --workspace -- -D warnings

fmt: ## Check formatting
	cargo fmt --all -- --check

fmt-fix: ## Auto-fix formatting
	cargo fmt --all

# ── Install ──────────────────────────────────────────────────────────
#
# Four supported flows (all build from source, then run `agent install`):
#
#   make install-user           # per-user install   (intended for macOS dev)
#   make install-user-pam       #    … with PAM login
#   make install-system         # system-wide install (intended for Linux)
#   make install-system-pam     #   … with PAM login
#
# PAM=1 also works on the base targets, e.g. `make install-user PAM=1`.
#
# PAM caveats:
#   - Linux needs the PAM dev headers at build time (see PAM var above).
#   - A Linux system install is what lets core verify other users' passwords;
#     for real PAM auth its service account must also be in the `shadow` group
#     (or use an SSSD stack). See docs/pam-authentication.md.

build-release: ## Build release binaries (PAM=1 compiles agentd-core with PAM)
	cargo build --release --workspace --bins
	@if [ "$(PAM)" = "1" ]; then \
		echo "==> Rebuilding agentd-core with PAM support (libpam dev headers required on Linux)"; \
		cargo build --release -p agentd-core --features pam; \
	fi

build-ui: ## Build the web UI assets (bun)
	cd ui && bun install --frozen-lockfile && bun run build

install-user: build-release build-ui ## Per-user install (macOS dev); PAM=1 for system-user login
	$(CLI_BIN) install --user --bin-src target/release --ui-dir $(UI_DIST)

install-system: build-release build-ui ## System-wide install (Linux); PAM=1 for system-user login
	$(CLI_BIN) install --system --bin-src target/release --ui-dir $(UI_DIST)

install-user-pam: ## Per-user install with PAM (macOS dev)
	$(MAKE) install-user PAM=1

install-system-pam: ## System-wide install with PAM (Linux)
	$(MAKE) install-system PAM=1

uninstall-user: ## Remove a per-user install (uses the installed `agent`)
	agent uninstall --user

uninstall-system: ## Remove a system-wide install (uses the installed `agent`)
	agent uninstall --system

# ── Docker ───────────────────────────────────────────────────────────

docker-build-claude: ## Build the Claude Code agent Docker image locally
	docker build -t $(CLAUDE_IMAGE) docker/claude-code/

docker-build-claude-multiarch: ## Build multi-platform Claude Code image (requires buildx)
	docker buildx build \
		--platform linux/amd64,linux/arm64 \
		-t $(CLAUDE_IMAGE) \
		docker/claude-code/

docker-run-claude: ## Run claude --version in the agent image (smoke test)
	docker run --rm $(CLAUDE_IMAGE) --version
