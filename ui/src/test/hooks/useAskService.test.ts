/**
 * Tests for useAskService hook.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { http, HttpResponse } from 'msw'
import { useAskService } from '@/hooks/useAskService'
import { server } from '@/test/mocks/server'
import { makeTriggerResponse, makeAnswerResponse, resetQuestionSeq } from '@/test/mocks/factories'

const BASE = 'http://localhost:17001'

beforeEach(() => {
  resetQuestionSeq()
})

afterEach(() => {
  vi.useRealTimers()
})

describe('useAskService', () => {
  describe('health', () => {
    it('reports reachable when health check succeeds', async () => {
      const { result } = renderHook(() => useAskService())
      await waitFor(() => expect(result.current.health.reachable).toBe(true))
      expect(result.current.health.checking).toBe(false)
    })

    it('reports unreachable when health check fails', async () => {
      server.use(http.get(`${BASE}/health`, () => HttpResponse.error()))
      const { result } = renderHook(() => useAskService())
      await waitFor(() => expect(result.current.health.checking).toBe(false))
      expect(result.current.health.reachable).toBe(false)
    })

    it('recheckHealth re-runs the health check', async () => {
      const { result } = renderHook(() => useAskService())
      await waitFor(() => expect(result.current.health.reachable).toBe(true))

      server.use(http.get(`${BASE}/health`, () => HttpResponse.error()))
      act(() => { result.current.recheckHealth() })
      await waitFor(() => expect(result.current.health.reachable).toBe(false))
    })
  })

  describe('runTrigger', () => {
    it('calls POST /trigger and sets result', async () => {
      const triggerData = makeTriggerResponse({
        checks_run: ['TmuxSessions'],
        notifications_sent: [],
      })
      server.use(http.post(`${BASE}/trigger`, () => HttpResponse.json(triggerData)))

      const { result } = renderHook(() => useAskService())
      await waitFor(() => expect(result.current.health.checking).toBe(false))

      await act(async () => { await result.current.runTrigger() })

      expect(result.current.lastTriggerResult).toEqual(triggerData)
      expect(result.current.lastTriggerAt).toBeDefined()
      expect(result.current.triggering).toBe(false)
    })

    it('sets triggerError when trigger fails', async () => {
      server.use(http.post(`${BASE}/trigger`, () => HttpResponse.error()))

      const { result } = renderHook(() => useAskService())
      await waitFor(() => expect(result.current.health.checking).toBe(false))

      await act(async () => { await result.current.runTrigger() })

      expect(result.current.triggerError).toBeDefined()
    })

    it('adds pending questions when notifications are sent', async () => {
      const triggerData = makeTriggerResponse({ notifications_sent: ['notif-abc'] })
      server.use(http.post(`${BASE}/trigger`, () => HttpResponse.json(triggerData)))

      const { result } = renderHook(() => useAskService())
      await waitFor(() => expect(result.current.health.checking).toBe(false))

      await act(async () => { await result.current.runTrigger() })

      await waitFor(() => expect(result.current.questions).toHaveLength(1))
      expect(result.current.questions[0].status).toBe('Pending')
      expect(result.current.questions[0].notification_id).toBe('notif-abc')
    })

    it('deduplicates questions by notification_id', async () => {
      const triggerData = makeTriggerResponse({ notifications_sent: ['notif-abc'] })
      server.use(http.post(`${BASE}/trigger`, () => HttpResponse.json(triggerData)))

      const { result } = renderHook(() => useAskService())
      await waitFor(() => expect(result.current.health.checking).toBe(false))

      await act(async () => { await result.current.runTrigger() })
      await act(async () => { await result.current.runTrigger() })

      await waitFor(() => expect(result.current.questions).toHaveLength(1))
    })
  })

  describe('submitAnswer', () => {
    it('marks question as Answered on success', async () => {
      const triggerData = makeTriggerResponse({ notifications_sent: ['notif-1'] })
      server.use(http.post(`${BASE}/trigger`, () => HttpResponse.json(triggerData)))
      server.use(
        http.post(`${BASE}/answer`, () =>
          HttpResponse.json(makeAnswerResponse({ success: true })),
        ),
      )

      const { result } = renderHook(() => useAskService())
      await waitFor(() => expect(result.current.health.checking).toBe(false))

      await act(async () => { await result.current.runTrigger() })
      await waitFor(() => expect(result.current.questions).toHaveLength(1))

      const questionId = result.current.questions[0].question_id
      let success = false
      await act(async () => {
        success = await result.current.submitAnswer(questionId, 'yes')
      })

      expect(success).toBe(true)
      await waitFor(() => expect(result.current.questions[0].status).toBe('Answered'))
    })

    it('sets answerError on failure', async () => {
      server.use(http.post(`${BASE}/answer`, () => HttpResponse.error()))

      const { result } = renderHook(() => useAskService())
      await waitFor(() => expect(result.current.health.checking).toBe(false))

      act(() => {
        result.current.addQuestion({
          question_id: 'q-1',
          notification_id: 'n-1',
          check_type: 'TmuxSessions',
          asked_at: '2024-01-01T00:00:00Z',
          status: 'Pending',
        })
      })

      let success = true
      await act(async () => {
        success = await result.current.submitAnswer('q-1', 'yes')
      })

      expect(success).toBe(false)
      expect(result.current.answerError).toBeDefined()
    })
  })

  describe('addQuestion', () => {
    it('adds a question to the list', async () => {
      const { result } = renderHook(() => useAskService())
      await waitFor(() => expect(result.current.health.checking).toBe(false))

      act(() => {
        result.current.addQuestion({
          question_id: 'q-1',
          notification_id: 'n-1',
          check_type: 'TmuxSessions',
          asked_at: '2024-01-01T00:00:00Z',
          status: 'Pending',
        })
      })
      await waitFor(() => expect(result.current.questions).toHaveLength(1))
    })

    it('does not add duplicate questions', async () => {
      const { result } = renderHook(() => useAskService())
      await waitFor(() => expect(result.current.health.checking).toBe(false))

      const q = {
        question_id: 'q-1',
        notification_id: 'n-1',
        check_type: 'TmuxSessions' as const,
        asked_at: '2024-01-01T00:00:00Z',
        status: 'Pending' as const,
      }
      act(() => {
        result.current.addQuestion(q)
        result.current.addQuestion(q)
      })
      await waitFor(() => expect(result.current.questions).toHaveLength(1))
    })
  })

  describe('auto-trigger', () => {
    it('starts with auto-trigger disabled', async () => {
      const { result } = renderHook(() => useAskService())
      await waitFor(() => expect(result.current.health.checking).toBe(false))
      expect(result.current.autoTrigger).toBe(false)
    })

    it('toggles auto-trigger on and off', async () => {
      const { result } = renderHook(() => useAskService())
      await waitFor(() => expect(result.current.health.checking).toBe(false))

      act(() => result.current.setAutoTrigger(true))
      expect(result.current.autoTrigger).toBe(true)
      act(() => result.current.setAutoTrigger(false))
      expect(result.current.autoTrigger).toBe(false)
    })

    it('updates interval', async () => {
      const { result } = renderHook(() => useAskService())
      await waitFor(() => expect(result.current.health.checking).toBe(false))

      act(() => result.current.setAutoTriggerInterval(300_000))
      expect(result.current.autoTriggerInterval).toBe(300_000)
    })
  })
})
