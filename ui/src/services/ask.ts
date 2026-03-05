/**
 * Client for the Ask service (default port 17001).
 *
 * Handles environment checks (tmux sessions, etc.) and the
 * question/answer workflow for interactive agent approvals.
 */

import { ApiClient } from './base'
import { serviceConfig } from './config'
import type { HealthResponse } from '@/types/common'
import type { AnswerRequest, AnswerResponse, TriggerResponse } from '@/types/ask'

export class AskClient extends ApiClient {
  // -------------------------------------------------------------------------
  // Health
  // -------------------------------------------------------------------------

  getHealth(): Promise<HealthResponse> {
    return this.get<HealthResponse>('/health')
  }

  // -------------------------------------------------------------------------
  // Environment checks
  // -------------------------------------------------------------------------

  /**
   * Run all environment checks (e.g., tmux session detection) and create
   * notifications for any actionable findings.
   */
  trigger(): Promise<TriggerResponse> {
    return this.post<TriggerResponse>('/trigger')
  }

  // -------------------------------------------------------------------------
  // Answers
  // -------------------------------------------------------------------------

  /**
   * Submit an answer to a pending question.
   */
  answer(request: AnswerRequest): Promise<AnswerResponse> {
    return this.post<AnswerResponse>('/answer', request)
  }
}

/** Singleton client instance using the configured service URL */
export const askClient = new AskClient({
  baseUrl: serviceConfig.askServiceUrl,
})
