/**
 * Unit tests for the test data factories.
 * Verifies that factories produce valid, typed objects.
 */

import { describe, it, expect, beforeEach } from 'vitest'
import {
  makeAgent,
  makeAgentList,
  makeAgentConfig,
  makePendingApproval,
  makeApprovalList,
  resetAgentSeq,
  makeNotification,
  makeUrgentNotification,
  makeNotificationList,
  makeCountResponse,
  resetNotificationSeq,
  makeQuestionInfo,
  makeTriggerResponse,
  makeAnswerResponse,
  resetQuestionSeq,
} from './index'

beforeEach(() => {
  resetAgentSeq()
  resetNotificationSeq()
  resetQuestionSeq()
})

// ---------------------------------------------------------------------------
// Agent factory
// ---------------------------------------------------------------------------

describe('makeAgentConfig', () => {
  it('returns a config with required fields', () => {
    const config = makeAgentConfig()
    expect(config.shell).toBe('/bin/bash')
    expect(config.tool_policy).toEqual({ type: 'AllowAll' })
  })

  it('applies overrides', () => {
    const config = makeAgentConfig({ interactive: true, model: 'claude-3-opus' })
    expect(config.interactive).toBe(true)
    expect(config.model).toBe('claude-3-opus')
  })
})

describe('makeAgent', () => {
  it('returns a valid agent with a unique id', () => {
    const a1 = makeAgent()
    const a2 = makeAgent()
    expect(a1.id).not.toBe(a2.id)
  })

  it('defaults to Running status', () => {
    expect(makeAgent().status).toBe('Running')
  })

  it('applies overrides', () => {
    const agent = makeAgent({ name: 'my-bot', status: 'Stopped' })
    expect(agent.name).toBe('my-bot')
    expect(agent.status).toBe('Stopped')
  })

  it('has an ISO 8601 created_at timestamp', () => {
    const agent = makeAgent()
    expect(new Date(agent.created_at).toISOString()).toBe(agent.created_at)
  })
})

describe('makeAgentList', () => {
  it('creates the requested number of agents', () => {
    expect(makeAgentList(5)).toHaveLength(5)
  })

  it('each agent has a unique id', () => {
    const ids = makeAgentList(4).map((a) => a.id)
    const unique = new Set(ids)
    expect(unique.size).toBe(4)
  })

  it('applies shared overrides to every agent', () => {
    const agents = makeAgentList(3, { status: 'Failed' })
    expect(agents.every((a) => a.status === 'Failed')).toBe(true)
  })
})

describe('makePendingApproval', () => {
  it('has Pending status by default', () => {
    expect(makePendingApproval().status).toBe('Pending')
  })

  it('applies overrides', () => {
    const approval = makePendingApproval({ tool_name: 'read_file' })
    expect(approval.tool_name).toBe('read_file')
  })
})

describe('makeApprovalList', () => {
  it('creates the requested number of approvals', () => {
    expect(makeApprovalList(2)).toHaveLength(2)
  })
})

// ---------------------------------------------------------------------------
// Notification factory
// ---------------------------------------------------------------------------

describe('makeNotification', () => {
  it('returns a valid notification with a unique id', () => {
    const n1 = makeNotification()
    const n2 = makeNotification()
    expect(n1.id).not.toBe(n2.id)
  })

  it('defaults to Normal priority and Pending status', () => {
    const notif = makeNotification()
    expect(notif.priority).toBe('Normal')
    expect(notif.status).toBe('Pending')
  })

  it('applies overrides', () => {
    const notif = makeNotification({ priority: 'High', title: 'Custom Title' })
    expect(notif.priority).toBe('High')
    expect(notif.title).toBe('Custom Title')
  })
})

describe('makeUrgentNotification', () => {
  it('sets priority to Urgent and requires_response to true', () => {
    const notif = makeUrgentNotification()
    expect(notif.priority).toBe('Urgent')
    expect(notif.requires_response).toBe(true)
  })
})

describe('makeNotificationList', () => {
  it('creates the requested number of notifications', () => {
    expect(makeNotificationList(7)).toHaveLength(7)
  })
})

describe('makeCountResponse', () => {
  it('returns a count response with correct total', () => {
    const resp = makeCountResponse(10, { Pending: 6, Viewed: 4 })
    expect(resp.total).toBe(10)
    expect(resp.by_status).toHaveLength(2)
  })
})

// ---------------------------------------------------------------------------
// Question/Ask factory
// ---------------------------------------------------------------------------

describe('makeQuestionInfo', () => {
  it('defaults to Pending status', () => {
    expect(makeQuestionInfo().status).toBe('Pending')
  })

  it('applies overrides', () => {
    const q = makeQuestionInfo({ status: 'Answered', answer: 'yes' })
    expect(q.status).toBe('Answered')
    expect(q.answer).toBe('yes')
  })
})

describe('makeTriggerResponse', () => {
  it('includes checks_run', () => {
    const resp = makeTriggerResponse()
    expect(resp.checks_run).toContain('TmuxSessions')
  })

  it('returns tmux_sessions result', () => {
    const resp = makeTriggerResponse()
    expect(resp.results.tmux_sessions.running).toBe(true)
  })

  it('applies overrides', () => {
    const resp = makeTriggerResponse({ notifications_sent: ['notif-1'] })
    expect(resp.notifications_sent).toContain('notif-1')
  })
})

describe('makeAnswerResponse', () => {
  it('defaults success to true', () => {
    expect(makeAnswerResponse().success).toBe(true)
  })
})
