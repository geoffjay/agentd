/**
 * Test data factory for Ask service types (QuestionInfo, TriggerResponse, etc.).
 *
 * Usage:
 *   const question = makeQuestionInfo()
 *   const trigger = makeTriggerResponse()
 */

import type { QuestionInfo, TriggerResponse, AnswerResponse } from '@/types/ask'

let _seq = 0
function nextId(): string {
  return String(++_seq)
}

/** Reset the sequence counter (call in beforeEach to get predictable IDs) */
export function resetQuestionSeq(): void {
  _seq = 0
}

// ---------------------------------------------------------------------------
// QuestionInfo factory
// ---------------------------------------------------------------------------

export function makeQuestionInfo(overrides?: Partial<QuestionInfo>): QuestionInfo {
  const id = nextId()
  return {
    question_id: id,
    notification_id: nextId(),
    check_type: 'TmuxSessions',
    asked_at: '2024-01-01T00:00:00Z',
    status: 'Pending',
    ...overrides,
  }
}

// ---------------------------------------------------------------------------
// TriggerResponse factory
// ---------------------------------------------------------------------------

export function makeTriggerResponse(overrides?: Partial<TriggerResponse>): TriggerResponse {
  return {
    checks_run: ['TmuxSessions'],
    notifications_sent: [],
    results: {
      tmux_sessions: {
        running: true,
        session_count: 2,
        sessions: ['main', 'dev'],
      },
    },
    ...overrides,
  }
}

// ---------------------------------------------------------------------------
// AnswerResponse factory
// ---------------------------------------------------------------------------

export function makeAnswerResponse(overrides?: Partial<AnswerResponse>): AnswerResponse {
  const id = nextId()
  return {
    success: true,
    message: 'Answer recorded successfully',
    question_id: id,
    ...overrides,
  }
}
