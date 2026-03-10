/**
 * TypeScript types for the Orchestrator service.
 * Mirrors the Rust types in crates/orchestrator.
 */

/** Lifecycle state of an agent */
export type AgentStatus = 'pending' | 'running' | 'stopped' | 'failed'

/** Approval lifecycle state */
export type ApprovalStatus = 'pending' | 'approved' | 'denied' | 'timed_out'

// ---------------------------------------------------------------------------
// ToolPolicy – discriminated union mirroring the Rust enum
// ---------------------------------------------------------------------------

export type ToolPolicy =
  | { mode: 'allow_all' }
  | { mode: 'deny_all' }
  | { mode: 'allow_list'; tools: string[] }
  | { mode: 'deny_list'; tools: string[] }
  | { mode: 'require_approval' }

// ---------------------------------------------------------------------------
// AgentConfig
// ---------------------------------------------------------------------------

/** Full agent configuration */
export interface AgentConfig {
  working_dir: string
  user?: string
  shell: string
  interactive: boolean
  prompt?: string
  worktree?: string
  system_prompt?: string
  tool_policy: ToolPolicy
  model?: string
  env?: Record<string, string>
  auto_clear_threshold?: number
}

// ---------------------------------------------------------------------------
// Agent / AgentResponse
// ---------------------------------------------------------------------------

/** Agent as returned by the API (env values are redacted) */
export interface Agent {
  id: string
  name: string
  status: AgentStatus
  config: AgentConfig
  session_id?: string
  backend_type?: string
  created_at: string
  updated_at: string
}

// ---------------------------------------------------------------------------
// Request bodies
// ---------------------------------------------------------------------------

/** Create-agent request: all AgentConfig fields plus a name */
export interface CreateAgentRequest {
  name: string
  working_dir: string
  user?: string
  shell: string
  interactive: boolean
  prompt?: string
  worktree?: string
  system_prompt?: string
  tool_policy: ToolPolicy
  model?: string
  env?: Record<string, string>
  auto_clear_threshold?: number
}

/** Send a message to an agent */
export interface SendMessageRequest {
  content: string
}

/** Response after sending a message */
export interface SendMessageResponse {
  status: string
  agent_id: string
}

/** Change the model used by an agent */
export interface SetModelRequest {
  model?: string
  restart: boolean
}

/** Update the tool policy for an agent */
export type UpdatePolicyRequest = ToolPolicy

// ---------------------------------------------------------------------------
// Approvals
// ---------------------------------------------------------------------------

/** A pending tool-use approval */
export interface PendingApproval {
  id: string
  agent_id: string
  request_id: string
  tool_name: string
  tool_input: unknown
  status: ApprovalStatus
  created_at: string
  expires_at: string
}

/** Body for approve/deny endpoints */
export interface ApprovalActionRequest {
  reason?: string
}

// ---------------------------------------------------------------------------
// Workflow / Task types (scheduler integration)
// ---------------------------------------------------------------------------

export type TaskStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled'

/** Status of a task dispatch record (mirrors Rust DispatchStatus) */
export type DispatchStatus = 'pending' | 'dispatched' | 'completed' | 'failed' | 'skipped'

/**
 * Tagged union for different task source backends.
 */
export type TaskSourceConfig =
  | {
      type: 'github_issues'
      owner: string
      repo: string
      labels: string[]
      state: 'open' | 'closed' | 'all'
    }
  | {
      type: 'github_pull_requests'
      owner: string
      repo: string
      labels: string[]
      state: 'open' | 'closed' | 'merged' | 'all'
    }

/**
 * A workflow as returned by the API.
 * Mirrors the Rust WorkflowResponse type.
 */
export interface Workflow {
  id: string
  name: string
  agent_id: string
  source_config: TaskSourceConfig
  prompt_template: string
  poll_interval_secs: number
  enabled: boolean
  tool_policy: ToolPolicy
  created_at: string
  updated_at: string
}

/**
 * A task dispatch record as returned by the API.
 * Mirrors the Rust DispatchResponse type.
 */
export interface DispatchRecord {
  id: string
  workflow_id: string
  source_id: string
  agent_id: string
  prompt_sent: string
  status: DispatchStatus
  dispatched_at: string
  completed_at?: string
}

/**
 * An external task fetched from a task source.
 * Mirrors the Rust Task type.
 */
export interface Task {
  source_id: string
  title: string
  body: string
  url: string
  labels: string[]
  assignee?: string
  metadata: Record<string, string>
}

/** Request body for creating a workflow */
export interface CreateWorkflowRequest {
  name: string
  agent_id: string
  source_config: TaskSourceConfig
  prompt_template: string
  poll_interval_secs: number
  enabled: boolean
  tool_policy: ToolPolicy
}

/** Request body for updating a workflow (all fields optional) */
export interface UpdateWorkflowRequest {
  name?: string
  prompt_template?: string
  poll_interval_secs?: number
  enabled?: boolean
  tool_policy?: ToolPolicy
}

/** Legacy type alias kept for compatibility */
export interface WorkflowConfig {
  name: string
  tasks: Array<{ type: string; [key: string]: unknown }>
}

// ---------------------------------------------------------------------------
// Usage tracking and context management
// ---------------------------------------------------------------------------

/** Token counts, cost, and timing from a single `result` message */
export interface UsageSnapshot {
  input_tokens: number
  output_tokens: number
  cache_read_input_tokens: number
  cache_creation_input_tokens: number
  total_cost_usd: number
  num_turns: number
  duration_ms: number
  duration_api_ms: number
}

/** Session-level aggregated usage */
export interface SessionUsage {
  input_tokens: number
  output_tokens: number
  cache_read_input_tokens: number
  cache_creation_input_tokens: number
  total_cost_usd: number
  num_turns: number
  duration_ms: number
  duration_api_ms: number
  result_count: number
  started_at: string
  ended_at?: string
}

/** Per-agent aggregated usage statistics */
export interface AgentUsageStats {
  agent_id: string
  current_session?: SessionUsage
  cumulative: SessionUsage
  session_count: number
}

/** Request body for POST /agents/{id}/clear-context */
export interface ClearContextRequest {}

/** Response body for POST /agents/{id}/clear-context */
export interface ClearContextResponse {
  agent_id: string
  session_usage?: SessionUsage
  new_session_number: number
}

// ---------------------------------------------------------------------------
// WebSocket event types
// ---------------------------------------------------------------------------

/** Agent produced a line of output on its log stream */
export interface AgentOutputEvent {
  type: 'agent:output'
  agentId: string
  line: string
  timestamp: string
}

/** Agent lifecycle state changed */
export interface AgentStatusChangeEvent {
  type: 'agent:status_change'
  agentId: string
  status: AgentStatus
  previousStatus?: AgentStatus
  timestamp: string
}

/** A new tool-use approval request arrived */
export interface ApprovalRequestedEvent {
  type: 'approval:requested'
  approval: PendingApproval
}

/** An approval was resolved (approved or denied) */
export interface ApprovalResolvedEvent {
  type: 'approval:resolved'
  approvalId: string
  status: 'approved' | 'denied'
  timestamp: string
}

/** A workflow task was dispatched to an agent */
export interface WorkflowTaskDispatchedEvent {
  type: 'workflow:task_dispatched'
  taskId: string
  agentId: string
  timestamp: string
}

/** A workflow task completed */
export interface WorkflowTaskCompletedEvent {
  type: 'workflow:task_completed'
  taskId: string
  result?: unknown
  timestamp: string
}

/** Real-time usage update for an agent (emitted after each result message) */
export interface UsageUpdateEvent {
  type: 'agent:usage_update'
  agentId: string
  usage: UsageSnapshot
  session_number: number
  timestamp: string
}

/** Agent context was cleared and a new session started */
export interface ContextClearedEvent {
  type: 'agent:context_cleared'
  agentId: string
  new_session_number: number
  previous_session_usage?: SessionUsage
  timestamp: string
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

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

export interface ListAgentsParams {
  status?: AgentStatus
  limit?: number
  offset?: number
}

export interface ListApprovalsParams {
  status?: ApprovalStatus
  limit?: number
  offset?: number
}
