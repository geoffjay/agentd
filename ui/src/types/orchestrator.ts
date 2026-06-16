/**
 * TypeScript types for the Orchestrator service.
 * Mirrors the Rust types in crates/orchestrator.
 */

/** Lifecycle state of an agent */
export type AgentStatus = "pending" | "running" | "stopped" | "failed";

/** Approval lifecycle state */
export type ApprovalStatus = "pending" | "approved" | "denied" | "timed_out";

// ---------------------------------------------------------------------------
// ToolPolicy – discriminated union mirroring the Rust enum
// ---------------------------------------------------------------------------

/**
 * Globs in `sandbox_bypass` are matched against tool+command patterns.
 * When a Bash call matches, the orchestrator auto-approves it with
 * `dangerouslyDisableSandbox: true` so the Claude Code sandbox is skipped
 * for that specific call (e.g. git-spice branch submit requires direct TLS).
 *
 * Glob syntax (same as the tools list):
 *   "Bash(git-spice *)"  — any git-spice command
 *   "Bash(gh pr *)"      — any gh pr subcommand
 */
export type ToolPolicy =
	| { mode: "allow_all"; sandbox_bypass?: string[] }
	| { mode: "deny_all"; sandbox_bypass?: string[] }
	| { mode: "allow_list"; tools: string[]; sandbox_bypass?: string[] }
	| { mode: "deny_list"; tools: string[]; sandbox_bypass?: string[] }
	| { mode: "require_approval"; sandbox_bypass?: string[] };

// ---------------------------------------------------------------------------
// AgentConfig
// ---------------------------------------------------------------------------

/** Docker network policy for container-backed agents. */
export type NetworkPolicy = "internet" | "isolated" | "host_network";

/** Additional volume mount for Docker-backed agents. */
export interface VolumeMount {
	host_path: string;
	container_path: string;
	read_only?: boolean;
}

/** CPU / memory limits for Docker-backed agents. */
export interface ResourceLimits {
	cpu_limit?: number;
	memory_limit_mb?: number;
}

/**
 * One stdio MCP server entry, mirroring Claude Code's `mcpServers` format.
 * `env` values are redacted in API responses like `AgentConfig.env`.
 */
export interface McpServerConfig {
	command: string;
	args?: string[];
	env?: Record<string, string>;
}

/** Full agent configuration */
export interface AgentConfig {
	working_dir: string;
	user?: string;
	shell: string;
	interactive: boolean;
	prompt?: string;
	/** When true, the session is started with --worktree. */
	worktree?: boolean;
	system_prompt?: string;
	/** Path to a file whose contents replace or append to the system prompt. */
	system_prompt_file?: string;
	/** When true, uses --append-system-prompt / --append-system-prompt-file instead of replacing. */
	append_system_prompt?: boolean;
	tool_policy: ToolPolicy;
	model?: string;
	env?: Record<string, string>;
	auto_clear_threshold?: number;
	additional_dirs?: string[];
	/** Communicate rooms the agent is auto-joined to on connection. */
	rooms?: string[];
	network_policy?: NetworkPolicy;
	docker_image?: string;
	extra_mounts?: VolumeMount[];
	resource_limits?: ResourceLimits;
	/** MCP servers for the agent's Claude session, keyed by server name. */
	mcp_servers?: Record<string, McpServerConfig>;
}

// ---------------------------------------------------------------------------
// Agent / AgentResponse
// ---------------------------------------------------------------------------

/** Agent as returned by the API (env values are redacted) */
export interface Agent {
	id: string;
	name: string;
	status: AgentStatus;
	config: AgentConfig;
	session_id?: string;
	backend_type?: string;
	/** The exact `claude` command used to launch this agent session. */
	launch_command?: string;
	/** OS process ID of the agent's subprocess. */
	pid?: number;
	/**
	 * Whether this is a built-in system agent.
	 *
	 * System agents are created programmatically at orchestrator startup and
	 * are always present. UI should hide destructive actions (delete, bulk-delete)
	 * for agents where this is `true`.
	 */
	built_in?: boolean;
	created_at: string;
	updated_at: string;
}

// ---------------------------------------------------------------------------
// Request bodies
// ---------------------------------------------------------------------------

/** Create-agent request: all AgentConfig fields plus a name */
export interface CreateAgentRequest {
	name: string;
	working_dir: string;
	user?: string;
	shell: string;
	interactive: boolean;
	prompt?: string;
	/** When true, the session is started with --worktree. */
	worktree?: boolean;
	system_prompt?: string;
	system_prompt_file?: string;
	append_system_prompt?: boolean;
	tool_policy: ToolPolicy;
	model?: string;
	env?: Record<string, string>;
	auto_clear_threshold?: number;
	additional_dirs?: string[];
	/** Communicate rooms the agent auto-joins when it connects. */
	rooms?: string[];
	network_policy?: NetworkPolicy;
	docker_image?: string;
	extra_mounts?: VolumeMount[];
	resource_limits?: ResourceLimits;
	/** MCP servers for the agent's Claude session, keyed by server name. */
	mcp_servers?: Record<string, McpServerConfig>;
}

/**
 * Value the API substitutes for env values in agent responses. Sending it
 * back in an update keeps the stored value for that key, so a redacted
 * config can be round-tripped without knowing the secrets.
 */
export const ENV_REDACTED = "***";

/**
 * Request body for PATCH /agents/{id} (merge-patch semantics).
 *
 * Absent fields are left unchanged. `env` is a full replacement when
 * present, except entries valued exactly `ENV_REDACTED` keep the stored
 * value. Empty strings clear `prompt` / `system_prompt` /
 * `system_prompt_file`; the two system prompt fields are mutually
 * exclusive (setting one non-empty clears the other).
 */
export interface UpdateAgentRequest {
	name?: string;
	working_dir?: string;
	shell?: string;
	prompt?: string;
	system_prompt?: string;
	system_prompt_file?: string;
	append_system_prompt?: boolean;
	model?: string;
	tool_policy?: ToolPolicy;
	env?: Record<string, string>;
	auto_clear_threshold?: number;
	additional_dirs?: string[];
	rooms?: string[];
	worktree?: boolean;
	/**
	 * Full replacement of the MCP server map when present (empty object
	 * clears). Entry env values equal to `ENV_REDACTED` keep the stored
	 * value, matching the `env` round-trip semantics.
	 */
	mcp_servers?: Record<string, McpServerConfig>;
	/**
	 * Restart the agent process immediately so launch-affecting changes
	 * (working_dir, shell, model, env, system prompt, additional_dirs,
	 * worktree, mcp_servers) take effect. Defaults to false.
	 */
	restart?: boolean;
}

/** Response body for PATCH /agents/{id}: the agent plus restart flags. */
export interface UpdateAgentResponse extends Agent {
	/**
	 * True when launch-affecting fields changed on a running agent and no
	 * restart was performed — the live process still uses the old config.
	 */
	requires_restart: boolean;
	/** True when the agent process was restarted as part of this update. */
	restarted: boolean;
}

/** Send a message to an agent */
export interface SendMessageRequest {
	content: string;
}

/** Response after sending a message */
export interface SendMessageResponse {
	status: string;
	agent_id: string;
}

/** Change the model used by an agent */
export interface SetModelRequest {
	model?: string;
	restart: boolean;
}

/** Update the tool policy for an agent */
export type UpdatePolicyRequest = ToolPolicy;

// ---------------------------------------------------------------------------
// Approvals
// ---------------------------------------------------------------------------

/** A pending tool-use approval */
export interface PendingApproval {
	id: string;
	agent_id: string;
	request_id: string;
	tool_name: string;
	tool_input: unknown;
	status: ApprovalStatus;
	created_at: string;
	expires_at: string;
}

/** Body for approve/deny endpoints */
export interface ApprovalActionRequest {
	reason?: string;
}

// ---------------------------------------------------------------------------
// Workflow / Task types (scheduler integration)
// ---------------------------------------------------------------------------

export type TaskStatus =
	| "pending"
	| "running"
	| "completed"
	| "failed"
	| "cancelled";

/** Status of a task dispatch record (mirrors Rust DispatchStatus) */
export type DispatchStatus =
	| "pending"
	| "dispatched"
	| "completed"
	| "failed"
	| "skipped";

/**
 * Expected origin of a webhook payload.
 * Mirrors the Rust WebhookSource enum (note: `GitHub` snake_cases to "git_hub").
 */
export type WebhookSource = "git_hub" | "linear" | "any";

/**
 * Tagged union for workflow trigger configurations.
 * Mirrors the Rust TriggerConfig enum (crates/orchestrator scheduler/types.rs).
 */
export type TriggerConfig =
	| {
			type: "github_issues";
			owner: string;
			repo: string;
			labels?: string[];
			state?: "open" | "closed" | "all";
			assignee?: string | null;
	  }
	| {
			type: "github_pull_requests";
			owner: string;
			repo: string;
			labels?: string[];
			state?: "open" | "closed" | "merged" | "all";
			assignee?: string | null;
	  }
	| {
			type: "cron";
			/** Standard cron expression, e.g. "0 9 * * MON-FRI". */
			expression: string;
	  }
	| {
			type: "delay";
			/** ISO 8601 datetime string. */
			run_at: string;
	  }
	| {
			type: "agent_lifecycle";
			event: "session_start" | "session_end" | "context_clear";
	  }
	| {
			type: "dispatch_result";
			source_workflow_id?: string | null;
			status?: DispatchStatus | null;
	  }
	| {
			type: "webhook";
			secret?: string | null;
			source?: WebhookSource;
	  }
	| { type: "manual" }
	| {
			type: "agent_idle";
			idle_seconds: number;
	  }
	| {
			type: "linear_issues";
			team_key?: string | null;
			project?: string | null;
			status?: string[] | null;
			labels?: string[];
			assignee?: string | null;
	  }
	| {
			type: "composite";
			mode: "or" | "and";
			/** Nested trigger configurations — minimum 2, max depth 3. */
			triggers: TriggerConfig[];
			/** AND mode: seconds before partial correlation state resets (default 60). */
			correlation_window_secs?: number | null;
	  }
	| {
			type: "queue";
			queue_name: string;
			poll_interval_secs?: number | null;
			visibility_timeout_secs?: number | null;
	  }
	| {
			type: "ask_response";
			agent_id?: string | null;
			category?: string | null;
			response_pattern?: string | null;
	  }
	| {
			type: "gitlab_issues";
			owner: string;
			repo: string;
			labels?: string[];
			state?: "opened" | "closed" | "all";
			assignee?: string | null;
	  }
	| {
			type: "gitlab_merge_requests";
			owner: string;
			repo: string;
			labels?: string[];
			state?: "opened" | "closed" | "merged" | "all";
			assignee?: string | null;
	  };

/** All trigger type discriminators. */
export type TriggerType = TriggerConfig["type"];

/**
 * A workflow as returned by the API.
 * Mirrors the Rust WorkflowResponse type.
 */
export interface Workflow {
	id: string;
	name: string;
	agent_id: string;
	trigger_config: TriggerConfig;
	prompt_template: string;
	poll_interval_secs: number;
	enabled: boolean;
	tool_policy: ToolPolicy;
	created_at: string;
	updated_at: string;
}

/**
 * A task dispatch record as returned by the API.
 * Mirrors the Rust DispatchResponse type.
 */
export interface DispatchRecord {
	id: string;
	workflow_id: string;
	source_id: string;
	agent_id: string;
	prompt_sent: string;
	status: DispatchStatus;
	dispatched_at: string;
	completed_at?: string;
	/**
	 * The task whose variables were rendered into prompt_sent.
	 * Absent for records created before task persistence was added.
	 */
	task?: Task | null;
}

/**
 * An external task fetched from a task source.
 * Mirrors the Rust Task type.
 */
export interface Task {
	source_id: string;
	title: string;
	body: string;
	url: string;
	labels: string[];
	assignee?: string;
	metadata: Record<string, string>;
}

/**
 * Request body for manually triggering a workflow.
 * Mirrors the Rust TriggerWorkflowRequest type.
 */
export interface TriggerWorkflowRequest {
	title?: string;
	body?: string;
	url?: string;
	labels?: string[];
	assignee?: string;
	metadata?: Record<string, string>;
}

/** Request body for creating a workflow */
export interface CreateWorkflowRequest {
	name: string;
	agent_id: string;
	trigger_config: TriggerConfig;
	prompt_template: string;
	poll_interval_secs: number;
	enabled: boolean;
	tool_policy: ToolPolicy;
}

/** Request body for updating a workflow (all fields optional) */
export interface UpdateWorkflowRequest {
	name?: string;
	prompt_template?: string;
	poll_interval_secs?: number;
	enabled?: boolean;
	tool_policy?: ToolPolicy;
	/**
	 * Replace the trigger configuration. If the workflow is enabled, the
	 * orchestrator restarts its runner so the change applies immediately.
	 */
	trigger_config?: TriggerConfig;
	/** Re-assign the workflow to a different agent (must be running). */
	agent_id?: string;
}

/** Legacy type alias kept for compatibility */
export interface WorkflowConfig {
	name: string;
	tasks: Array<{ type: string; [key: string]: unknown }>;
}

// ---------------------------------------------------------------------------
// Usage tracking and context management
// ---------------------------------------------------------------------------

/** Token counts, cost, and timing from a single `result` message */
export interface UsageSnapshot {
	input_tokens: number;
	output_tokens: number;
	cache_read_input_tokens: number;
	cache_creation_input_tokens: number;
	total_cost_usd: number;
	num_turns: number;
	duration_ms: number;
	duration_api_ms: number;
}

/** Session-level aggregated usage */
export interface SessionUsage {
	input_tokens: number;
	output_tokens: number;
	cache_read_input_tokens: number;
	cache_creation_input_tokens: number;
	total_cost_usd: number;
	num_turns: number;
	duration_ms: number;
	duration_api_ms: number;
	result_count: number;
	started_at: string;
	ended_at?: string;
}

/** Per-agent aggregated usage statistics */
export interface AgentUsageStats {
	agent_id: string;
	current_session?: SessionUsage;
	cumulative: SessionUsage;
	session_count: number;
}

/** Request body for POST/DELETE /agents/{id}/dirs */
export interface AddDirRequest {
	path: string;
}

/** Response body for POST/DELETE /agents/{id}/dirs */
export interface AddDirResponse {
	agent_id: string;
	additional_dirs: string[];
	requires_restart: boolean;
}

/** Request body for POST /agents/{id}/clear-context */
export type ClearContextRequest = {};

/** Response body for POST /agents/{id}/clear-context */
export interface ClearContextResponse {
	agent_id: string;
	session_usage?: SessionUsage;
	new_session_number: number;
}

// ---------------------------------------------------------------------------
// WebSocket event types
// ---------------------------------------------------------------------------

/** Agent produced a line of output on its log stream */
export interface AgentOutputEvent {
	type: "agent:output";
	agentId: string;
	line: string;
	timestamp: string;
}

/** Agent lifecycle state changed */
export interface AgentStatusChangeEvent {
	type: "agent:status_change";
	agentId: string;
	status: AgentStatus;
	previousStatus?: AgentStatus;
	timestamp: string;
}

/** A new tool-use approval request arrived */
export interface ApprovalRequestedEvent {
	type: "approval:requested";
	approval: PendingApproval;
}

/** An approval was resolved (approved or denied) */
export interface ApprovalResolvedEvent {
	type: "approval:resolved";
	approvalId: string;
	status: "approved" | "denied";
	timestamp: string;
}

/** A workflow task was dispatched to an agent */
export interface WorkflowTaskDispatchedEvent {
	type: "workflow:task_dispatched";
	taskId: string;
	agentId: string;
	timestamp: string;
}

/** A workflow task completed */
export interface WorkflowTaskCompletedEvent {
	type: "workflow:task_completed";
	taskId: string;
	result?: unknown;
	timestamp: string;
}

/** Real-time usage update for an agent (emitted after each result message) */
export interface UsageUpdateEvent {
	type: "agent:usage_update";
	agentId: string;
	usage: UsageSnapshot;
	session_number: number;
	timestamp: string;
}

/** Agent context was cleared and a new session started */
export interface ContextClearedEvent {
	type: "agent:context_cleared";
	agentId: string;
	new_session_number: number;
	previous_session_usage?: SessionUsage;
	timestamp: string;
}

/** Agent invoked a tool — carries full input and a human-readable summary */
export interface AgentToolUseEvent {
	type: "agent:tool_use";
	agentId: string;
	tool_name: string;
	tool_id: string;
	tool_input: Record<string, unknown>;
	summary: string;
	timestamp: string;
}

/** Agent emitted a thinking/reasoning block */
export interface AgentThinkingEvent {
	type: "agent:thinking";
	agentId: string;
	text: string;
	timestamp: string;
}

/** Union of all agent-related WebSocket events */
export type AgentEvent =
	| AgentOutputEvent
	| AgentStatusChangeEvent
	| ApprovalRequestedEvent
	| ApprovalResolvedEvent
	| WorkflowTaskDispatchedEvent
	| WorkflowTaskCompletedEvent
	| UsageUpdateEvent
	| ContextClearedEvent
	| AgentToolUseEvent
	| AgentThinkingEvent;

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

export interface ListAgentsParams {
	status?: AgentStatus;
	limit?: number;
	offset?: number;
}

export interface ListApprovalsParams {
	status?: ApprovalStatus;
	limit?: number;
	offset?: number;
}

// ---------------------------------------------------------------------------
// Projects
// ---------------------------------------------------------------------------

/** A project as returned by the orchestrator API. */
export interface Project {
	id: string;
	name: string;
	description: string | null;
	created_at: string;
	updated_at: string;
}

/** Query parameters for `GET /projects`. */
export interface ListProjectsParams {
	limit?: number;
	offset?: number;
}

