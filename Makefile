# agentd — top-level build automation
#
# Usage:
#   make help              Show available targets
#   make build             Build the Rust workspace
#   make test              Run all tests
#   make docker-build-claude  Build the Claude Code Docker image locally

.PHONY: help build test clippy fmt docker-build-claude docker-run-claude

# Default image name — matches the DEFAULT_IMAGE constant in crates/wrap/src/docker.rs
CLAUDE_IMAGE ?= agentd-claude:latest

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
