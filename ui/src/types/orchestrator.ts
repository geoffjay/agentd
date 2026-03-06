/**
 * TypeScript types for the Orchestrator service.
 * Mirrors the Rust types in crates/orchestrator.
 */

/** Lifecycle state of an agent */
export type AgentStatus = 'Pending' | 'Running' | 'Stopped' | 'Failed'

/** Approval lifecycle state */
export type ApprovalStatus = 'Pending' | 'Approved' | 'Denied' | 'TimedOut'

// ---------------------------------------------------------------------------
// ToolPolicy – discriminated union mirroring the Rust enum
// ---------------------------------------------------------------------------

export type ToolPolicy =
  | { type: 'AllowAll' }
  | { type: 'DenyAll' }
  | { type: 'AllowList'; tools: string[] }
  | { type: 'DenyList'; tools: string[] }
  | { type: 'RequireApproval' }

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
  tmux_session?: string
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

export type TaskStatus = 'Pending' | 'Running' | 'Completed' | 'Failed' | 'Cancelled'

/** Status of a task dispatch record (mirrors Rust DispatchStatus) */
export type DispatchStatus = 'pending' | 'dispatched' | 'completed' | 'failed' | 'skipped'

/**
 * Tagged union for different task source backends.
 * Currently only GitHub Issues is supported.
 */
export type TaskSourceConfig =
  | {
      type: 'github_issues'
      owner: string
      repo: string
      labels: string[]
      state: 'open' | 'closed' | 'all'
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
  status: 'Approved' | 'Denied'
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

/** Union of all agent-related WebSocket events */
export type AgentEvent =
  | AgentOutputEvent
  | AgentStatusChangeEvent
  | ApprovalRequestedEvent
  | ApprovalResolvedEvent
  | WorkflowTaskDispatchedEvent
  | WorkflowTaskCompletedEvent

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
