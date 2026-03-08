/**
 * TypeScript types for the Notify service.
 * Mirrors the Rust types in crates/notify.
 */

// ---------------------------------------------------------------------------
// Enums / discriminated unions
// ---------------------------------------------------------------------------

/** Where the notification originated */
export type NotificationSource = 'AgentHook' | 'AskService' | 'MonitorService' | 'System'

/** How long the notification lives */
export type NotificationLifetime =
  | { type: 'persistent' }
  | { type: 'ephemeral'; expires_at: string }

/** Display priority */
export type NotificationPriority = 'low' | 'normal' | 'high' | 'urgent'

/** Current state of a notification */
export type NotificationStatus = 'pending' | 'viewed' | 'responded' | 'dismissed' | 'expired'

// ---------------------------------------------------------------------------
// Notification model
// ---------------------------------------------------------------------------

export interface Notification {
  id: string
  source: NotificationSource
  lifetime: NotificationLifetime
  priority: NotificationPriority
  status: NotificationStatus
  title: string
  message: string
  requires_response: boolean
  response?: string
  created_at: string
  updated_at: string
}

// ---------------------------------------------------------------------------
// Request bodies
// ---------------------------------------------------------------------------

export interface CreateNotificationRequest {
  source: NotificationSource
  lifetime: NotificationLifetime
  priority: NotificationPriority
  title: string
  message: string
  requires_response: boolean
}

export interface UpdateNotificationRequest {
  status?: NotificationStatus
  response?: string
}

// ---------------------------------------------------------------------------
// Count response
// ---------------------------------------------------------------------------

export interface StatusCount {
  status: string
  count: number
}

export interface CountResponse {
  total: number
  by_status: StatusCount[]
}

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

export interface ListNotificationsParams {
  status?: NotificationStatus
  limit?: number
  offset?: number
}
