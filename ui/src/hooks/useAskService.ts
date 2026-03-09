/**
 * useAskService — state management for the Ask service.
 *
 * Provides:
 * - Service health (reachability, connected notify URL)
 * - Trigger: run environment checks, track results and history
 * - Auto-trigger: optional interval-based triggering
 * - Questions: the list of QuestionInfo items from recent triggers
 * - Answer: submit answers to pending questions
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { askClient } from '@/services/ask'
import type { QuestionInfo, TriggerResponse } from '@/types/ask'
import type { HealthResponse } from '@/types/common'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface AskServiceHealth {
  reachable: boolean
  checking: boolean
  /** URL the ask service uses to reach notify (from /health response) */
  notifyUrl?: string
  version?: string
}

export type AutoTriggerInterval = 30_000 | 60_000 | 300_000 | 600_000

export interface UseAskServiceResult {
  // Health
  health: AskServiceHealth
  recheckHealth: () => void

  // Triggering
  triggering: boolean
  lastTriggerResult?: TriggerResponse
  lastTriggerAt?: Date
  triggerError?: string
  runTrigger: () => Promise<void>

  // Auto-trigger
  autoTrigger: boolean
  autoTriggerInterval: AutoTriggerInterval
  setAutoTrigger: (enabled: boolean) => void
  setAutoTriggerInterval: (ms: AutoTriggerInterval) => void

  // Questions (derived from triggers + manual additions)
  questions: QuestionInfo[]
  addQuestion: (q: QuestionInfo) => void

  // Answering
  answering: boolean
  answerError?: string
  submitAnswer: (questionId: string, answer: string) => Promise<boolean>
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

const AUTO_TRIGGER_OPTIONS: AutoTriggerInterval[] = [30_000, 60_000, 300_000, 600_000]
export { AUTO_TRIGGER_OPTIONS }

export function useAskService(): UseAskServiceResult {
  const [health, setHealth] = useState<AskServiceHealth>({ reachable: false, checking: true })
  const [triggering, setTriggering] = useState(false)
  const [lastTriggerResult, setLastTriggerResult] = useState<TriggerResponse | undefined>()
  const [lastTriggerAt, setLastTriggerAt] = useState<Date | undefined>()
  const [triggerError, setTriggerError] = useState<string | undefined>()
  const [autoTrigger, setAutoTrigger] = useState(false)
  const [autoTriggerInterval, setAutoTriggerInterval] = useState<AutoTriggerInterval>(60_000)
  const [questions, setQuestions] = useState<QuestionInfo[]>([])
  const [answering, setAnswering] = useState(false)
  const [answerError, setAnswerError] = useState<string | undefined>()

  const autoTimerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // -------------------------------------------------------------------------
  // Health check
  // -------------------------------------------------------------------------

  const recheckHealth = useCallback(async () => {
    setHealth((prev) => ({ ...prev, checking: true }))
    try {
      const res = await askClient.getHealth() as HealthResponse & { notify_url?: string }
      setHealth({
        reachable: true,
        checking: false,
        notifyUrl: res.notify_url,
        version: res.version,
      })
    } catch {
      setHealth({ reachable: false, checking: false })
    }
  }, [])

  useEffect(() => {
    void recheckHealth()
  }, [recheckHealth])

  // -------------------------------------------------------------------------
  // Trigger
  // -------------------------------------------------------------------------

  const runTrigger = useCallback(async () => {
    setTriggering(true)
    setTriggerError(undefined)
    try {
      const result = await askClient.trigger()
      setLastTriggerResult(result)
      setLastTriggerAt(new Date())

      // Extract question info from trigger results if notifications were sent.
      // The ask service creates questions client-side based on notifications_sent
      // since the trigger endpoint doesn't return QuestionInfo directly.
      if (result.notifications_sent.length > 0) {
        const newQuestions: QuestionInfo[] = result.notifications_sent.map((notifId) => ({
          question_id: crypto.randomUUID(),
          notification_id: notifId,
          check_type: 'TmuxSessions',
          asked_at: new Date().toISOString(),
          status: 'Pending' as const,
        }))
        setQuestions((prev) => {
          // Deduplicate by notification_id
          const existingNotifIds = new Set(prev.map((q) => q.notification_id))
          const deduped = newQuestions.filter((q) => !existingNotifIds.has(q.notification_id))
          return [...deduped, ...prev]
        })
      }
    } catch (err) {
      setTriggerError(err instanceof Error ? err.message : 'Failed to run checks')
    } finally {
      setTriggering(false)
    }
  }, [])

  // -------------------------------------------------------------------------
  // Auto-trigger timer
  // -------------------------------------------------------------------------

  useEffect(() => {
    if (autoTimerRef.current) clearInterval(autoTimerRef.current)
    if (!autoTrigger) return
    autoTimerRef.current = setInterval(() => void runTrigger(), autoTriggerInterval)
    return () => {
      if (autoTimerRef.current) clearInterval(autoTimerRef.current)
    }
  }, [autoTrigger, autoTriggerInterval, runTrigger])

  // -------------------------------------------------------------------------
  // Questions
  // -------------------------------------------------------------------------

  const addQuestion = useCallback((q: QuestionInfo) => {
    setQuestions((prev) => {
      if (prev.some((existing) => existing.question_id === q.question_id)) return prev
      return [q, ...prev]
    })
  }, [])

  // -------------------------------------------------------------------------
  // Answer
  // -------------------------------------------------------------------------

  const submitAnswer = useCallback(async (questionId: string, answer: string): Promise<boolean> => {
    setAnswering(true)
    setAnswerError(undefined)
    try {
      const res = await askClient.answer({ question_id: questionId, answer })
      if (res.success) {
        setQuestions((prev) =>
          prev.map((q) =>
            q.question_id === questionId
              ? { ...q, status: 'Answered' as const, answer }
              : q,
          ),
        )
        return true
      } else {
        setAnswerError(res.message)
        return false
      }
    } catch (err) {
      setAnswerError(err instanceof Error ? err.message : 'Failed to submit answer')
      return false
    } finally {
      setAnswering(false)
    }
  }, [])

  return {
    health,
    recheckHealth,
    triggering,
    lastTriggerResult,
    lastTriggerAt,
    triggerError,
    runTrigger,
    autoTrigger,
    autoTriggerInterval,
    setAutoTrigger,
    setAutoTriggerInterval,
    questions,
    addQuestion,
    answering,
    answerError,
    submitAnswer,
  }
}
