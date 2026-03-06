/**
 * TypeScript types for the Hook service.
 *
 * The hook service (port 17002) monitors git hooks and system hooks and
 * creates notifications based on hook events. These types define the
 * anticipated data model for when the service is fully implemented.
 */

// ---------------------------------------------------------------------------
// Hook type / event model
// ---------------------------------------------------------------------------

/** Category of hook source */
export type HookType = 'git' | 'system'

/** Status of an individual hook definition */
export type HookStatus = 'active' | 'inactive' | 'error'

/**
 * A registered hook that the hook service monitors.
 *
 * Git hooks: pre-commit, post-commit, pre-push, post-merge, etc.
 * System hooks: file-change, process-event, cron-trigger, etc.
 */
export interface Hook {
  id: string
  name: string
  type: HookType
  /** Specific event that triggers this hook (e.g. "pre-commit", "file-change") */
  event: string
  enabled: boolean
  created_at: string
  updated_at?: string
  /** Human-readable description of what this hook does */
  description?: string
  /** Notification message template rendered when the hook fires */
  notification_template?: string
}

/**
 * A log entry representing a single hook execution event.
 */
export interface HookEvent {
  id: string
  hook_id: string
  hook_name: string
  hook_type: HookType
  event: string
  status: 'success' | 'failure' | 'skipped'
  triggered_at: string
  duration_ms?: number
  payload?: Record<string, unknown>
  error?: string
}

/**
 * Configuration for a hook's notification trigger.
 */
export interface HookNotificationTrigger {
  hook_id: string
  /** Which hook events produce notifications: "all" | "failure" | "success" */
  on: 'all' | 'failure' | 'success'
  title_template: string
  message_template: string
  priority: 'Low' | 'Normal' | 'High' | 'Urgent'
}

// ---------------------------------------------------------------------------
// API response shapes (anticipated)
// ---------------------------------------------------------------------------

export interface HookListResponse {
  hooks: Hook[]
  total: number
}

export interface HookEventListResponse {
  events: HookEvent[]
  total: number
}
