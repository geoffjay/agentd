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

export interface TaskSourceConfig {
  type: string
  [key: string]: unknown
}

export interface Task {
  id: string
  workflow_id: string
  name: string
  status: TaskStatus
  source: TaskSourceConfig
  created_at: string
  updated_at: string
}

export interface WorkflowConfig {
  name: string
  tasks: TaskSourceConfig[]
}

export interface DispatchRecord {
  id: string
  workflow_id: string
  dispatched_at: string
  status: TaskStatus
}

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
