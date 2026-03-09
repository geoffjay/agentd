/**
 * TypeScript types for the Ask service.
 * Mirrors the Rust types in crates/ask.
 */

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/** The category of environment check */
export type CheckType = 'TmuxSessions'

/** Current state of a question */
export type QuestionStatus = 'Pending' | 'Answered' | 'Expired'

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

/** Info about an outstanding question */
export interface QuestionInfo {
  question_id: string
  notification_id: string
  check_type: CheckType
  asked_at: string
  status: QuestionStatus
  answer?: string
}

/** Result of a tmux-sessions check */
export interface TmuxCheckResult {
  running: boolean
  session_count: number
  sessions?: string[]
}

/** Aggregated trigger results keyed by check type */
export interface TriggerResults {
  tmux_sessions: TmuxCheckResult
}

/** Response from POST /trigger */
export interface TriggerResponse {
  checks_run: string[]
  notifications_sent: string[]
  results: TriggerResults
}

// ---------------------------------------------------------------------------
// Request / response bodies
// ---------------------------------------------------------------------------

export interface AnswerRequest {
  question_id: string
  answer: string
}

export interface AnswerResponse {
  success: boolean
  message: string
  question_id: string
}
