# Changelog

All notable changes to this project will be documented in this file.
Versions follow [Semantic Versioning](https://semver.org/).

## [0.4.13] - 2026-06-21

### Added

- Add MCP tools for knowledgebase
- Add memory tool
- Add CLI integration tests
- Add CLI integration tests
- Linux-only implementation
- Support macos pam
- Support PAM in build
- Add PAM config to core and validate auth
- Add Makefile targets

### Fixed

- Update trigger_config in tools
- Update memory service for missing config
- Login background layout flash

## [0.4.12] - 2026-06-17

### Added

- KB-1 Foundation agentd-knowledge crate scaffold
- KB-1 foundation — crate scaffold, KnowledgeConfig, service registration
- Register knowledge in core gateway + restore KnowledgeConfig (F1)
- KB-2 storage and filesystem layer
- Implement storage and filesystem layer (KB-2)
- Add optional org-scope filtering to storage queries (F3a)
- KB-3 REST API handlers
- Implement REST API layer (KB-3)
- Scope REST handlers by X-Tenant-ID via core gateway (F3b)
- KB-4 client and CLI commands
- KB-4 client and CLI commands
- Route CLI/client through core gateway with bearer auth (F2)
- KB-5 knowledgebase UI
- KB-5 knowledgebase UI
- KB-6 doctor reconciliation and docs
- KB-6 doctor endpoint, CLI subcommand, env docs, E2E tests
- KB-7 tenant scoping — NULL-row transition policy and gateway docs
- KB-7 tenant scoping — NULL-row transition policy and gateway docs
- Add CLI config section
- Improve kb layout

### Fixed

- Address PR #1300 review feedback
- Address KB-5 review feedback
- Handle config correctly in core
- Clippy warnings
- Route knowledge client through core gateway
- Add knowledge to Procfile, drop removed index crate
- Resolve failure on document creation

## [0.4.11] - 2026-06-16

### Added

- Add install method to CLI install

### Fixed

- Handle ws connections through core

## [0.4.10] - 2026-06-16

### Fixed

- Expose core service in runtime /config.json

## [0.4.9] - 2026-06-15

### Added

- Auth integration foundation — gateway completeness, tenant extractor, token validation
- Gateway completeness, TenantId extractor, POST /auth/validate, Procfile
- Route all commands through core gateway with bearer tokens
- Route all commands through core gateway with bearer tokens
- Add login page, auth store, gateway routing, WebSocket auth
- Add auth store, login/register pages, gateway routing, WebSocket auth
- Downstream service tenant scoping with organization_id and WebSocket auth
- Add organization_id tenant scoping to downstream services with WS auth
- Add admin backfill-tenant command for organization_id migration
- Add admin backfill-tenant command for organization_id migration
- Auth cleanup - proxy simplification, tenant propagation, MCP docs
- Simplify proxy to single gateway route
- Add rollback_migrations helper to agentd_common storage
- Add rollback_migrations helper to agentd_common::storage
- Expose migration helpers from agentd-core lib.rs
- Expose apply/status/rollback migration helpers from lib.rs
- Register agentd-core in xtask DB_SERVICES for migration management
- Register agentd-core in DB_SERVICES for migration management
- Add clap CLI with migrate subcommand to agentd-core binary
- Add clap CLI with migrate subcommand to agentd-core binary
- Add product superuser role and admin entity views
- Add set-password subcommand

### Fixed

- Address PR #1288 review feedback
- Address review feedback on org_id scoping
- Address review feedback on backfill-tenant command
- Replace atty with std::io::IsTerminal, use workspace clap
- Change install resolution function
- Prevent session masking invalid login
- Allow superuser role to access admin
- Update ui tests
- Add zustand as dep
- Drop intel darwin from cache warm job
- Improve test coverage

## [0.4.8] - 2026-06-12

### Fixed

- Resolve agentd CLI as 'cli' before 'agent' for MCP server command
- Wire permission prompts for subprocess stdio agents
- Terminate stdio initialize handshake with a newline

## [0.4.7] - 2026-06-12

### Added

- General agent update endpoint and workflow trigger updates
- Full TriggerConfig types and trigger_config rename
- Dedicated agent and workflow create/edit pages
- YAML template import/export on the form pages
- System-agent registry with config refresh and lazy spawn
- Generate the system prompt service table from config
- Trailing-* glob for plain tool-name policy patterns
- Mcp_servers agent config with --mcp-config launch support
- Agentd-diagnostician built-in agent
- Agent and workflow creation tools, ToolPolicy wire-shape fix
- Agentd-architect built-in agent
- Prometheus query client and curated named-query API
- Query_metrics tool backed by the monitor named-query API
- Agentd-analyst built-in agent
- Monitor named-query client and live System Resources charts
- Platform Metrics section from the monitor named-query catalog

### Documentation

- System agents expansion tracking document

### Fixed

- Let agent and workflow form pages fill the content width
- Scrape monitor Prometheus metrics from /prom-metrics
- Add the CORS layer to the HTTP router
- Support message submit for agent in a pending state

## [0.4.6] - 2026-06-11

### Changed

- Split memory API surface into a memory-api crate

## [0.4.5] - 2026-06-11

### Added

- Persist task variables on dispatch records
- Re-trigger workflow dispatches from the dispatch history
- Runtime service configuration via /config.json

## [0.4.4-rc.1] - 2026-06-10

### Added

- Overhaul dashboard with theme-aware charts, auto-refresh, and system metrics
- Release tarballs, checksums, and curl|sh installer (phase 2)

## [0.4.3] - 2026-06-10

### Added

- Update crush config

## [0.4.2] - 2026-06-08

### Added

- Extract install/service logic into agentd-install crate

### Fixed

- Use up to date intel runner

## [0.4.1] - 2026-06-07

### Fixed

- Use rustls reqwest in all crates to fix musl release build

## [0.4.0] - 2026-06-07

### Added

- Implement PtyBackend for ExecutionBackend trait (closes #673)
- PTY session I/O capture and streaming infrastructure
- Add PtyOutputStream with ring-buffer history and broadcast streaming
- Add PTY stream relay WebSocket endpoint to orchestrator
- Add PTY terminal relay WebSocket endpoint (closes #675)
- Add xterm.js web terminal component for agent sessions
- Add xterm.js terminal component for agent PTY sessions
- CLI backend selection and PTY session management commands
- Backend selection configuration and documentation
- Backend selection configuration and documentation (closes #678)
- Add PTY mode badge and SDK-mode info banner to AgentTerminal
- Add PTY mode badge and dismissible SDK-mode info banner to AgentTerminal
- Add interactive-mode toggle to agent creation form
- Add interactive-mode toggle to Advanced section of agent creation form (closes #775)
- Display tool policy as list in agent details
- Display launch command on agent details page
- Display launch command on agent details page (closes #784)
- Use HighlightedCode for launch command display
- Use HighlightedCode for launch command and redact system prompt (closes #790)
- Support system_prompt_file and append_system_prompt in AgentConfig
- Support system_prompt_file and append_system_prompt in AgentConfig (closes #792)
- Add Linear API key configuration support
- Add Linear API key configuration support (closes #474)
- Implement LinearIssueSource task source
- Implement LinearIssueSource task source (closes #475)
- Wire Linear trigger into scheduler and API validation
- Add Linear trigger filter validation in API (closes #476)
- Add Linear trigger type to CLI workflow commands
- Add linear-issues trigger type to workflow commands (closes #477)
- Add Linear-specific template variables
- Add Linear-specific template variables (closes #478)
- Add Linear webhook payload parsing
- Add Linear webhook payload parsing
- Add YAML schema files
- Use schema with templates
- Improve layout
- Add epic workflow
- Add prompt command with @ parsing and tests
- Add prompt command with @ parsing, name matching, and tests
- Implement recipient resolution with agent/room matching
- Implement recipient resolution with agent/room matching
- Implement message routing to agents and rooms
- Implement message routing to agents and rooms
- Add biome
- Add agentd-ui service and install support
- Add dialoguer dependency and interactive recipient picker
- Add dialoguer dependency and interactive recipient picker
- Scaffold agentd-mcp crate with rmcp server framework
- Scaffold agentd-mcp crate with rmcp server framework
- Implement diagnostic tools for agentd-mcp
- Implement diagnostic tools for agentd-mcp
- Implement approval management tools
- Implement approval management tools
- Implement agent lifecycle management tools
- Implement agent lifecycle management tools
- Implement service health and system metrics tools
- Implement service health and system metrics tools
- Implement agent inspection tools
- Implement agent inspection tools (#250)
- Implement workflow and dispatch inspection tools
- Implement workflow and dispatch inspection tools (#251)
- Implement notification inspection and management tools
- Implement notification inspection and management tools (#252)
- Implement self-healing remediation tools (#257)
- Add frontend test job with codecov coverage
- Add Prometheus config and launchd plist for local scraping
- Add Grafana provisioning, dashboards, and launchd plist
- Add setup script, teardown script, and unified observability docs
- Copy config on install
- Add mcp subcommand
- Implement IdleStrategy with configurable idle timeout
- Implement IdleStrategy with configurable idle timeout (closes #807)
- Add AgentIdle variant to TriggerConfig and wire into strategy factory
- Add AgentIdle variant to TriggerConfig and wire into strategy factory (closes #808)
- Add tests and documentation for AgentIdle trigger
- Add tests and documentation for AgentIdle trigger (closes #809)
- Add mcp config
- Implement unimplemented metrics
- Implement metrics and dashboards
- Add update script
- Implement OR combinator CompositeStrategy
- Implement OR combinator CompositeStrategy (closes #814)
- Implement AND combinator with correlation window
- Implement AND combinator with correlation window (closes #815)
- Add Composite variant to TriggerConfig with nested config
- Add Composite variant to TriggerConfig with nested config support (closes #816)
- Add integration tests and docs for composite triggers
- Add tests and documentation for composite triggers (closes #817)
- Add queue table migration and storage operations for task queue
- Add task_queue table migration and storage operations for queue-based trigger
- Implement QueueStrategy consuming tasks from internal queue
- Implement QueueStrategy consuming tasks from internal queue
- Add queue API endpoints for push/stats/peek/purge
- Register queue routes and add push/stats/peek/purge handlers
- Add Queue variant to TriggerConfig and wire into API/CLI
- Add Queue trigger type wired into strategy, API, and CLI
- Add SQLite persistence with SeaORM
- Add SQLite persistence with SeaORM (#870)
- Extensible check type registry
- Extensible check type registry (#872)
- List endpoint for questions
- List and get question endpoints
- Improve theme support
- Redesign ask service data model for agent-driven questions
- Redesign data model for agent-driven Q&A
- Remove check/trigger system from ask service
- Remove check/trigger system
- Implement new ask service REST API for agent questions
- Implement new REST API for agent-driven Q&A
- Add ask event callback to orchestrator for workflow triggers
- Add ask event callback endpoint and AskResponseReceived event
- Add ask_response workflow trigger type to orchestrator scheduler
- Add ask_response workflow trigger type
- Update ask service client and CLI commands for new Q&A model
- Implement full AskClient and CLI Q&A commands (#925)
- Create crates/index crate scaffold and workspace integration
- Create crates/index crate scaffold and workspace integration (#938)
- Add index service to xtask and platform service management
- Add index service to platform service management
- Full configuration system with environment variables
- Full configuration system with environment variables
- Tree-sitter syntactic chunking engine
- Tree-sitter syntactic chunking engine
- Semantic chunking with docstring and signature grouping
- Semantic chunking with docstring and signature grouping
- Hierarchical indexing with multi-level summaries
- Hierarchical indexing with multi-level summaries
- LanceDB code chunk schema and Ollama embedding integration
- LanceDB code chunk schema and Ollama embedding integration
- Incremental indexing with file hashing and change detection
- Structural metadata extraction pipeline
- Structural metadata extraction pipeline (#946)
- LLM-generated natural language summaries for code blocks
- LLM-generated natural language summaries for code blocks (#947)
- Dependency mapping for imports and cross-file relationships
- Dependency mapping for imports and cross-file relationships (#948)
- Vector similarity search endpoint for code retrieval
- Vector similarity search endpoint for code retrieval (#949)
- Hybrid search combining vector similarity with BM25 keyword matching
- Hybrid search combining vector similarity with BM25 keyword matching (#950)
- Search reranking with cross-encoder model and agentic search fallback
- Search reranking with cross-encoder model and agentic search fallback (#951)
- Repository management API and file system watcher
- Repository management API and file system watcher
- CLI agent index subcommands
- Add agent index subcommands
- Agent-index Claude Code skill
- Add agent-index Claude Code skill
- Use index protocol
- Update TypeScript types to match new Question model
- Update TypeScript types to match new Question model
- Rewrite AskClient to call new /questions/* endpoints
- Rewrite AskClient to call new /questions/* endpoints
- Rewrite useAskService hook for question fetching
- Rewrite useAskService hook for question fetching
- Rebuild QuestionCard and AnswerDialog for new model
- Rebuild QuestionCard and AnswerDialog for new Question model
- Rebuild QuestionsPage with filters, pagination, and dismiss
- Rebuild QuestionsPage with filters, pagination, and dismiss
- Add question detail view with dedicated route
- Add question detail view at /questions/:id
- Comprehensive tests for new ask UI components
- Add index service proxy route to UI backend crate
- Add index service proxy route to UI backend crate
- Add TypeScript types and API client for index service
- Add TypeScript types and API client for index service
- Build useIndexService hook for repository and search state
- Build useIndexService hook for repository and search state
- Build IndexPage with repository management and search
- Add URL param sync and missing search filters to IndexPage
- Search results DataTable with code preview drawer
- Add SearchResultsTable and CodePreviewDrawer for code search results
- Comprehensive tests for index service UI
- Add comprehensive tests for index service UI
- Vector embedding scatter plot for search results
- Add SearchScatterPlot for search result score visualisation
- Cluster density map for repository embeddings
- Cluster density map for repository embedding health
- Hex-bin density heatmap for large indices
- Add hex-bin density heatmap for large indices
- Add Swift language support to index service
- Add Swift language support with tree-sitter chunking
- Add Zig language support with tree-sitter chunking
- Add Go language support with tree-sitter chunking
- Add ruby support to index
- Add sandbox_bypass to tool_policy schema and agent details UI
- Add sandbox_bypass to ToolPolicy for TLS-sensitive commands
- Auto-scroll to bottom when opening a communication room
- Auto-scroll to bottom when opening a communication room
- Add Index and Communicate service health cards to dashboard
- Add Index and Communicate service health cards to dashboard
- Add follow-conversation button when scrolled up in chat rooms
- Add follow-conversation button when scrolled up in chat rooms
- Show agent thinking/working indicator in communication rooms
- Show agent thinking/working indicator in communication rooms
- Add subprocess backend
- Add env for path
- Set env and sandbox disable in agents
- Agent launch improvements
- Add agent restart
- Add restart to agent details page
- Scaffold core service crate
- Scaffold agentd-core service with health endpoint and Prometheus metrics
- Set up entity module structure and storage layer for core crate
- Add entity module stubs, migration runner, and Storage wrapper
- Implement sea-orm migration for core crate schema
- Implement initial sea-orm migration for core service schema
- Define SeaORM User entity for core crate
- Define SeaORM User entity with has_many relations and test
- Define SeaORM Session entity for authentication
- Define SeaORM Session entity with belongs_to User and tests
- Define SeaORM Organization and Membership entities with relations
- Define Organization and Membership entities with many-to-many relations
- Add SeaORM integration tests for core crate entity relations
- Add SeaORM integration tests for entity relations
- Add OrganizationStorage and MembershipStorage to core service
- Add OrganizationStorage and MembershipStorage with CRUD and tests
- Add UserStorage with CRUD and argon2 password hashing
- Add UserStorage with argon2 password hashing and username migration
- Add authentication endpoints
- Add authentication endpoints with SQLite-backed sessions
- Add tenant context middleware and active organization endpoint
- Add TenantContext middleware and active organization endpoint
- Add user and organization management API endpoints
- Add user and organization management API endpoints
- Add auth and org commands for core service
- Add auth and org commands for core service
- Add API gateway proxy to core service
- Add API gateway proxy for downstream service routing
- Add projects table, entity, and CRUD storage to orchestrator
- Add projects table, entity, and CRUD storage (#827)
- Add project_id FK to agents, workflows, and rooms tables
- Add project_id FK to agents, workflows, and rooms tables
- Add project CRUD API endpoints
- Add project CRUD API endpoints
- Add project subcommand for project management
- Add project subcommand for project management
- Add built_in flag to agent schema
- Add built_in flag to agent schema and storage layer
- Filter built-in agents from list API, add /system-agents endpoint
- Filter built-in agents from list API, add /system-agents endpoint
- Define system agent config and spawn at startup
- Define system agent config and spawn at startup
- Finalize system agent system prompt and tool policy
- Finalize system agent system prompt and tool policy
- Add system-agents subcommand
- Add system-agents subcommand
- Add system agents section to agents page
- Add system agents section to agents page
- Improve agents page
- Make wrap service ring buffer configurable
- Make PTY ring-buffer configurable via env vars (address review feedback)
- Add conversation_events table and storage
- Persist agent stream events in WebSocket handler
- Add REST API endpoints for conversation history
- Load persisted conversation history in agent detail view
- Add conversation retention policy and cleanup
- Add conversation retention policy and periodic cleanup (address review feedback)
- Add release automation with changelog generation
- Add release automation with changelog generation (address review feedback)
- Add test coverage
- Add pkgbuild
- Add index-service feature gate to cli and xtask crates
- Improve linux install
- Migrate subprocess backend from --sdk-url to stdin/stdout NDJSON
- Support bind address
- Add GitLab config, trigger type variants, and module scaffolding
- Rename assignees→assignee on GitlabMergeRequests, move test helpers to cfg(test), document GitLab template variables
- Add assignee/assignees filter fields to GitHub triggers
- Rename assignees→assignee on GithubPullRequests, add is_draft/head_ref/base_ref to KNOWN_VARIABLES
- Add build cache
- Add rework workflow and plan skill
- Add config module with TOML schema and layered loading
- Add config module with TOML schema and layered loading
- Add config init and config show subcommands
- Add config init and config show subcommands (#1195)
- Add config structs for services with inline env var config
- Add ValidateConfig trait and per-service validation
- Support new message streams
- Add TUI app
- TUI agent message input improvements
- Improve TUI message input
- Improve workflows view in TUI
- Improve TUI UX
- Fix port settings
- Add agent control command
- Add TUI config
- Add manager app
- Display system agents in TUI agent panel
- Improve MCP tools
- Unify Web UI and TUI conversation stream via v2 protocol
- Add hk for pre-commit
- Use different favicon
- Add release skill

### Changed

- Migrate services with existing config.rs to shared TOML config

### Documentation

- Add Linear trigger documentation and example workflows
- Fix factual inaccuracies flagged in review
- Add README and MCP client configuration guide
- Add README and MCP client configuration guide (#259)
- Create agentd-mcp implementation plan
- Add agentd-mcp implementation plan (closes #248)
- Update agent-ask skill for new Q&A paradigm
- Rewrite agent-ask skill for Q&A paradigm (#926)
- Add example ask workflows for Q&A patterns
- Add example ask workflows for Q&A patterns (#927)
- Index service documentation
- Add service documentation and update README
- Add ADR 0001 (agentd-mcp stdio transport) and wire into nav
- Add ADR 0001 (agentd-mcp stdio transport) and wire into nav
- Add docker-backend.md to mkdocs nav and prepare for pty backend
- Add Execution Backends nav section with docker-backend.md
- Update schema and docs for GitLab triggers
- Update schema and docs for GitLab triggers and GitHub assignee fields
- Add comprehensive configuration system documentation

### Fixed

- Fix pty_stream doctest by removing tokio_test dependency
- Auto-focus terminal on interactive mode toggle
- Auto-focus terminal when switching to interactive mode
- PTY stdin prompt injection for interactive-mode agents
- PTY stdin injection for interactive-mode agents
- Route terminal input correctly based on agent mode
- Route AgentTerminal input based on agent mode
- PTY backend skips --sdk-url for interactive sessions
- PTY backend skips --sdk-url for interactive sessions
- Persist effective_interactive to agent record
- Persist effective_interactive to agent record (closes #781)
- Handle PTY backend in attach command
- Handle PTY backend in attach command (closes #782)
- Add policy display component
- Regenerate Cargo.lock to fix corrupted merge conflict resolution
- Address PR #800 review feedback
- Resolve all -D warnings to unblock CI
- Apply review feedback
- Update example templates
- Use bun instead of npm for UI build
- Run biome format
- Run biome format
- Ensure PR bodies use dedicated Closes #N line for GitHub issue linking
- Use explicit PR body with dedicated Closes #N line for GitHub issue linking
- Em-dashes are dumb
- Agent template indentation
- Fix clippy warnings in CompositeStrategy and API validation
- Wire queue task lifecycle into notify_complete
- Pass None storage arg to create_strategy in composite trigger tests
- Handle agent disconnection and reconciliation
- Change memory API sort order
- Update frontend tests
- Remove memory consumption error
- Probe actual embedding dimension instead of using static lookup
- Probe actual embedding dimension instead of using static lookup
- Remove nomic-embed-code because it's not real
- Clippy errors
- Make all tests pass
- Map backend questions field to PaginatedResponse items
- Answer and dismiss endpoints return Question not ActionResponse
- Align search mode and repo status casing with backend
- Cache LanceDB table handle to prevent EMFILE under concurrent load
- Open file issue with lance
- Raise RLIMIT_NOFILE and add search semaphore to prevent EMFILE
- Correct total_chunks count and switch to canvas scatter plot
- Correct total_chunks count and switch health chart to canvas
- Update cost calculation
- Remove unused binsParam from renderHeatMap
- Prevent infinite remount
- Update hexbin counters
- Run cargo fmt
- Clippy errors
- Clippy errors
- Agent spawn timing issue
- Dashboard Create Agent button is non-functional
- Wire up dashboard Create Agent button to open dialog
- Guard source_config access in WorkflowForm
- Guard source_config access in WorkflowForm to prevent TypeError
- Build issue
- Use char-boundary-aware truncation in summarize_tool_input
- Address PR review on conversation storage foundation
- Fix session init off-by-one and add persistence tests
- Address PR review - session filter, summary aggregation, cursor encoding
- Clippy errors
- Failing tests
- Run cargo fmt
- Cargo audit changes
- Clippy issue
- Clippy issue
- Add llvm for tarpaulin
- Clippy issue
- Frontend timeout issue in test
- Resolve ui build issues
- Add --verbose required by --output-format=stream-json with --print
- Run cargo fmt
- Remove dead DrainLevel::Debug variant, simplify drain task
- Suppress large_enum_variant lint on SessionState
- Update rustls-webpki 0.103.12 -> 0.103.13 (RUSTSEC-2026-0104)
- Handle user and env
- Strip sudo vars when user is provided
- Don't blindly accept every call
- Update failing tests
- Assignee not assignees
- Run cargo fmt
- Run clippy
- Don't build index by default
- Resolve AGENTD_HISTORY_SIZE collision, complete env-var docs, add missing schema fields
- Fix overwrite-guard tests to call cmd_init; clean up config_file_path indirection
- Fix ENV_LOCK races and AGENTD_PORT env override in config tests
- Use unwrap_or_else with tracing::warn for load() failures; add TODO(#1201) comments
- Use unwrap_or_else with tracing::warn for config load failures
- Address review feedback on ValidateConfig PR
- Run clippy
- Service connection in TUI
- Prioritize config for service execution
- Add promql metrics to TUI
- Make config consistent with env
- Restart workflow runners when an agent is restarted
- Address review feedback
- TUI reconnects on disconnect; Web UI always full-snapshots on mount
- Use ratatui's true wrap row count so follow mode tracks the tail
- Ui formatting
- Clippy and test issues
- Doc_test issues

## [0.3.0-pre] - 2026-03-23

### Added

- Migrate agentd-common storage module from SQLx to SeaORM
- Define SeaORM entities and migrate notify storage to SeaORM
- Define SeaORM entities for orchestrator crate agent and scheduler models
- Remove deprecated SQLx dependencies after SeaORM migration
- Add sea-orm-cli entity generation and migration commands to xtask
- Generate db entities
- Support pull requests in workflows
- Add review agent out of staging
- Reconcile restarts agents with stale tmux sessions on startup
- Add Otterfile
- Add agents and skills for claude code
- Update Otterfile
- Add usage tracking and context management types
- Add agent_usage_sessions schema and auto_clear_threshold
- Add usage session CRUD storage methods (#147)
- Add clear_context and get_usage_stats to AgentManager (#150)
- Add usage and clear-context endpoints
- Add usage and clear-context client methods
- Add usage and clear-context subcommands
- Add usage persistence and auto-clear callback
- Extract usage data from WebSocket result messages
- Support auto_clear_threshold in YAML agent templates
- Add TypeScript types for usage tracking and context management
- Add usage API client methods and useAgentUsage hook
- Add agent usage panel to agent detail page
- Add clear context action and auto-clear threshold config
- Add usage and cache efficiency charts to monitoring dashboard
- Add real-time usage update events to WebSocket stream
- Add usage cost indicators to agent list and dashboard summary
- Define ExecutionBackend trait and implement TmuxBackend adapter
- Add debug endpoint and websocket improvements
- Implement DockerBackend using bollard crate
- Handle Docker networking and WebSocket URL rewriting
- Add documentation agent and workflow
- Add Dockerfile and CI pipeline for Claude Code execution image
- Add design agent and workflow
- Add backend selection to AgentConfig and orchestrator startup
- Update CLI for Docker backend support
- Add design file
- Improve layout and style
- Make input multi-line
- Update design files
- Add reconciliation and health check support for Docker backend
- Add integration tests and documentation for Docker execution backend
- Support AGENTD_ENV for dev and test
- Add additional_dirs field to AgentConfig, DB entity, and YAML template
- Add REST API endpoints to manage agent additional_dirs at runtime ([#363](https://github.com/geoffjay/agentd/pull/363))
- Shell-escape --add-dir paths and warn on missing dirs
- Add add-dir and remove-dir commands for agent directory management
- Display and manage additional directories in agent details page
- Consolidate agent action buttons into a dropdown menu
- Scaffold agentd-memory crate with workspace integration (#300)
- Define memory types, traits, and domain model (#301)
- Implement embedding service with OpenAI-compatible provider support
- Implement LanceDB vector store backend
- Implement REST API endpoints for memory service
- Add CLI commands and client for agentd-memory service
- Add integration tests and comprehensive documentation
- Add memory TypeScript types and API client
- Add useMemories and useMemorySearch React hooks
- Add memory list page with filters and pagination
- Add memory create dialog and semantic search panel
- Add memory plist
- Integrate memory service into navigation, search, and dashboard
- Add memory UI integration tests and MSW test infrastructure
- Implement SQLite metadata storage and SeaORM entities
- Consistent table layouts and detail drawer for list pages
- Define TriggerStrategy trait for workflow scheduling
- Implement PollingStrategy wrapping TaskSource + interval
- Add POST /workflows/{id}/trigger endpoint for manual dispatch
- Add protection from destructive commands
- Add skills
- Add memory protocol to agents and workflows
- Add Bash(pattern) command-level filtering to tool policies
- Enhance agent log tool call display with tool details
- Persist agent log history and surface thinking blocks
- Track agent busy/idle activity state in ConnectionRegistry
- Add lsp for claude
- Add LinearIssues variant to TriggerConfig
- Define room/participant/message models, entities, and migrations
- Scaffold communicate crate with Axum server and SeaORM storage
- Implement room management REST API (#491)
- Implement participant management REST API (#492)
- Implement message persistence and REST API (#493)
- Add real-time WebSocket message streaming (#494)
- Add CommunicateClient for service-to-service calls
- Auto-register agents as room participants on connection (#496)
- Implement MessageBridge for agent message delivery (#497)
- Add room management endpoints for agents
- Add communicate subcommand group for room and message management
- Add room templates and communicate integration to agent apply
- Implement room list and chat message view for inter-agent comms
- Add communicate service to migration pipeline
- Add plist for communicate
- Add human message input and room management to communicate
- Add integration tests for message bridge (#504)
- Improve chat room layout
- Add agent-communicate Claude Code skill
- Update agents with rooms
- Add real-time todos panel to agent details page
- Show empty todos
- Add syntax highlighting for prompt display via react-shiki (#507)
- Initialize git-spice and configure project defaults
- Add git-spice skill with project conventions
- Update all agent system prompts to use git-spice commands
- Update workflow prompt templates to use git-spice commands
- Add operations and security communication rooms
- Add conductor agent for pipeline orchestration
- Add triage agent for automatic issue processing
- Add enricher agent for issue quality improvement
- Add tester agent for systematic test coverage
- Add security agent for proactive vulnerability auditing
- Add refactor agent and workflows for code improvement
- Add research agent for technology investigation and analysis
- Add enrichment-worker workflow (closes #629)
- Add test-worker workflow (closes #630)
- Add research-worker workflow (closes #632)
- Add architect agent for cross-service design review and ADRs
- Add release-manager agent for changelog and version management
- Add security-audit and security-worker workflows (closes #633)
- Update room memberships for all new agents (#636)
- Create pipeline labels and define label-driven state machine
- Create pipeline labels and define label-driven state machine
- Create research-agent GitHub label
- Create research-agent label and add declarative label config
- Create merge-worker merge orchestration workflow
- Create merge-worker merge orchestration workflow
- Create conductor-sync pipeline orchestration workflow
- Create conductor-sync pipeline orchestration workflows
- Create triage-worker workflow
- Create triage-worker workflow for needs-triage label dispatch
- Add file-path scoping to restrict agent write access
- Add file-path scoping to agent tool policies
- Add dispatch_result source_id propagation and wire pipeline chaining workflows
- Add dispatch_result trigger and source_id propagation (address review feedback)
- Define and enforce human approval gates for autonomous operations
- Define and enforce human approval gates
- Add merge automation to conductor agent
- Add detailed merge automation with CI verification and stack ordering
- Add agent coordination hooks to prevent duplicate work
- Agent coordination checkpoint (address review feedback)
- Add HealthResponse::degraded constructor and is_healthy helper to common crate
- Add HealthResponse::degraded constructor and is_healthy helper (closes #647)

### Changed

- Rename tmux_session to session_id with storage migration
- Make bollard a regular dependency instead of feature-gated
- WorkflowRunner accepts Box<dyn TriggerStrategy>

### Documentation

- Add SeaORM patterns and conventions for agentd contributors (#230)
- Documentation review and update for issue #318
- Add additional directories feature documentation (#361)
- Add memory service API and CLI documentation
- Document TriggerStrategy abstraction and source_config migration (#349)
- Document cron and delay schedule triggers (#350)
- Document event bus and lifecycle/dispatch-result triggers (#351)
- Document webhook trigger setup and GitHub integration (#352)
- Document manual trigger API and CLI usage (#353)
- Review and consolidate workflow trigger documentation (#348)
- Document inter-agent communication architecture and user guide (#505)
- Clarify dev vs prod port scheme and agent status behaviour
- Add research agent reference and pipeline flow
- Add research agent reference and update nav (closes #683)
- Autonomous pipeline architecture reference
- Add autonomous pipeline architecture reference (closes #646)
- Add agent contributor onboarding guide
- Add agent contributor onboarding guide (closes #649)

### Fixed

- Run cargo fmt
- Run cargo fmt
- Resolve clippy errors
- Run cargo fmt
- Remove duplication
- Run cargo fmt
- Remove generated entities
- Ui layout issues
- Use snake case on agent status
- Fix notifications page crash caused by NotificationSource type mismatch
- Add useEffect return check
- Terminate_agent deletes storage record instead of updating to Stopped
- Run cargo fmt
- Run cargo fmt
- Resolve clippy errors
- Remove broken width setting from dialog
- Address review feedback
- Run cargo fmt
- Update quinn-proto
- Tweak layout and font colors
- Apply review feedback
- Handle tmux errors correctly
- Run cargo fmt
- Resolve review feedback
- Resolve review feedback
- Run cargo fmt
- Run cargo fmt
- Run cargo fmt
- Layout issues
- Add usage to agent list
- Handle websocket log events
- Reconnect restarted agents
- Handle restart reconnection
- Run cargo fmt
- Run cargo fmt
- Address PR #313 review feedback
- Run cargo fmt
- Rename existing node user instead of creating new UID 1000
- Address code review findings for Dockerfile and CI
- Run cargo fmt
- Address PR #324 code review feedback
- Run cargo fmt
- Run cargo fmt
- Address PR #325 code review feedback
- Run cargo fmt
- Use /bin/sh instead of /bin/bash for general agent type in Docker
- Allocate TTY and open stdin for Docker containers
- Dialog width
- Remove stale types.rs placeholder after rebase onto feature/memory-service
- Run cargo fmt
- Clippy errors
- Clippy errors
- Address PR #377 review feedback
- Run cargo fmt
- Use consistent port pattern
- Memory dialog width
- Drawer width
- Run cargo fmt
- Run cargo fmt
- Install protoc for lance-encoding build on Linux
- Resolve clippy warnings for too_many_arguments and large_enum_variant
- Run cargo fmt
- Run cargo fmt
- Run cargo fmt
- Run cargo fmt
- Run cargo fmt
- Run cargo fmt
- Run cargo fmt
- Run cargo fmt
- Run cargo fmt
- Hook path and cleanup issue
- Improper handling of undefined
- Address review feedback on MessageBridge (#497)
- Address PR #517 review — typed communicate errors
- Errors in corrupt details log
- Address PR #518 review — watch auto-join, port, URL encoding, pagination
- Address PR #519 review — teardown rooms, conflict handling, UX fixes
- Address PR #521 review — openWebSocket recursion, socket event handling, useMemo
- Layout
- Address PR #524 review feedback
- Template kind for rooms ([#525](https://github.com/geoffjay/agentd/pull/525))
- Address PR #526 review feedback
- Handle chat room websocket correctly
- Prevent undefined access
- Make communication page fixed
- Remove annoying border
- More layout
- Add port to orchestrator plist
- Persist and restore agent room memberships
- Deduplicate message delivery to agents via MessageBridge
- Remove failing use of mut
- Address PR #482 review issues
- Resolve cargo audit vulnerabilities
- Resolve cargo audit failures on issue-473
- Correct grep comment filters and audit redundancy in security-audit workflow
- Address review feedback on Step 1, 3, 6, 8 and conductor sort note
- Normalize room participant identifiers to agent UUIDs
- Normalize room participant identifiers to agent UUIDs (closes #707)
- Resolve cargo audit security vulnerabilities
- Resolve cargo audit security vulnerabilities ([#713](https://github.com/geoffjay/agentd/pull/713))
- Resolve new agent stack issues
- Reduce conductor command size using scripts

## [0.3.0] - 2026-03-08

### Added

- Add orchestrator to xtask service management and CLI
- Add GitHub Actions CI/CD pipeline
- Add cross-platform support to xtask service management
- Add session listing and status endpoints to wrap crate
- Add pagination to list endpoints in notify and orchestrator
- Add test coverage for baml and cli crates
- Add structured error handling and logging standards
- Add tool-use policy enforcement to orchestrator WebSocket handler
- Improve create-agent CLI command usability
- Add shell completion generation to CLI
- Create typed OrchestratorClient to replace generic ApiClient
- Add health, attach, stream, and send-message commands using typed OrchestratorClient
- Add CLI integration and audit logging for ToolPolicy enforcement
- Add Prometheus metrics endpoints for observability
- Add human-in-the-loop tool approval for RequireApproval policy
- Add workflow template examples and validation
- Add tool policy to workflow definitions for scheduled agents
- Add composite apply/teardown commands for .agentd/ project directories
- Migrate launch scripts to .agentd/ YAML templates
- Create agentd-common crate skeleton for shared types and utilities
- Add model selection to agent creation (#114)
- Add ability to change the model of a running agent (#115)
- Add env field to AgentConfig and CreateAgentRequest
- Inject env vars into agent tmux session at launch
- Add --env flag to create-agent command
- Add env field tests and example template for agent apply
- Add review agent
- Add review agent
- Initialize frontend project with React, Vite, Tailwind, and Bun
- Add TypeScript API client layer for all backend services
- Add app shell layout with fixed header, collapsible sidebar, and routing
- Add dashboard home page with service health, agent/notification summary, and activity feed
- Build settings page with service config and UI preferences (#174)
- Implement global search palette (#175)
- Set up frontend testing infrastructure with MSW, factories, and a11y tests (#178)
- Add dark mode theme support with system preference detection (#180)
- Build agents list view with filtering, status indicators, and creation
- Build agent detail view with live log streaming and command input
- Build tool approval queue UI with notifications (#173)
- Implement WebSocket infrastructure for real-time agent streaming
- Run prettier
- Apply CORS layer to all backend service routers
- Build notifications view with filtering, priority sorting, and action handling
- Implement global error handling with error boundaries and toast notifications
- Implement accessibility compliance and responsive design (#177)
- Build questions view for ask service with trigger controls and answer submission (#170)
- Build monitoring dashboard with Nivo charts and placeholder metrics (#171)
- Build workflow management view for automated task dispatching (#172)
- Build hooks management page with placeholder for hook service (#179)
- Implement system monitoring daemon with REST API
- Add simplify that never got committed
- Add structured error handling to hook and monitor stub crates
- Implement hook crate shell event monitoring daemon
- Add a worker for planning
- Add sea-orm and sea-orm-migration workspace dependencies

### Changed

- Standardize axum version across all service crates
- Extract shared pagination types into agentd-common crate
- Extract shared SQLite storage utilities into agentd-common
- Extract server init and tracing setup into agentd-common
- Deduplicate ApiError types into agentd-common crate
- Standardize HealthResponse types across all services
- Remove TODO comment, tracked as issue #159

### Documentation

- Update README with orchestrator, wrap, and scheduler services
- Fix port number inconsistencies across all documentation
- Add configuration and environment variable reference guide
- Add end-to-end getting started guide for new users
- Add API reference documentation for ask and wrap services
- Update README to reflect current project state and feature set
- Document new CLI commands and update usage examples
- Document agent apply command and .agentd/ template convention
- Document tool policies and human-in-the-loop approvals
- Add crate-level documentation to all workspace crates

### Fixed

- Remove duplicate request types in notify crate
- Remove zensical to use mkdocs config
- Run cargo fmt
- Update CI for clippy/audit/test
- Run cargo fmt
- Copy pasta from different repo
- Update test that fails in CI only
- Update more failing tests
- Run cargo fmt
- Add cargo audit file
- Run cargo fmt
- Resolve clippy issues
- Add var for launch scripts
- Resolve gh path
- Use paginated response
- Fix and improve create-workflow CLI command
- Run cargo fmt
- Resolve cargo clippy errors
- Run cargo fmt
- Merge failure
- Clippy error
- Run cargo fmt
- Run cargo fmt
- Remove unused import
- Run cargo fmt
- Run cargo fmt
- Resolve clippy errors
- Run cargo fmt
- Run cargo fmt
- Remove unresolved import
- Run cargo fmt
- Run cargo fmt
- Fix scheduler notify_task_complete matching wrong workflow runner
- Fix notification priority ordering in SQLite queries
- Fmt and clippy
- Run cargo fmt
- Allow single agent apply
- Run cargo fmt
- Run cargo fmt
- Run cargo fmt
- Run cargo fmt
- Sign bundle
- Websocket issues
- Run cargo fmt
- Run cargo fmt

## [0.2.0] - 2026-03-02

### Added

- Implement CLI, ask service, and macOS app bundle
- Add JSON output support and kanagawa theme library
- Implement complete palette API and theme system
- Add assets
- Create minimal UI framework
- Add notification service connection
- Add settings dialog, polling, and table enhancements
- Add wrap service with tmux integration
- Improve interface and add terminal
- Implement orchestrator
- Add agent arguments
- Implement orchestrator workflows
- Handle websocket communication
- Add agent launch scripts

### Changed

- Convert to pure REST API service
- Simplify crate names and update documentation
- Consolidate types and create domain-specific clients

### Fixed

- Correct axum routes, ports, and installation issues
- Align serde attributes with CLI for JSON compatibility
- Resolve connection issues
- Run cargo fmt
- Handle agent messaging correctly

<!-- generated by git-cliff -->
