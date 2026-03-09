/**
 * Tests for QuestionCard component.
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { QuestionCard } from '@/components/questions/QuestionCard'
import type { QuestionInfo } from '@/types/ask'

function makeQuestion(overrides: Partial<QuestionInfo> = {}): QuestionInfo {
  return {
    question_id: 'q-1',
    notification_id: 'notif-abc-123',
    check_type: 'TmuxSessions',
    asked_at: '2024-06-01T12:00:00Z',
    status: 'Pending',
    ...overrides,
  }
}

describe('QuestionCard', () => {
  it('renders the check type label', () => {
    render(<QuestionCard question={makeQuestion()} onAnswer={vi.fn()} />)
    expect(screen.getByText('Tmux Sessions')).toBeTruthy()
  })

  it('renders the notification ID', () => {
    render(<QuestionCard question={makeQuestion()} onAnswer={vi.fn()} />)
    expect(screen.getByText('notif-abc-123')).toBeTruthy()
  })

  it('shows "Pending" status badge', () => {
    render(<QuestionCard question={makeQuestion({ status: 'Pending' })} onAnswer={vi.fn()} />)
    expect(screen.getByText('Pending')).toBeTruthy()
  })

  it('shows "Answered" status badge', () => {
    render(<QuestionCard question={makeQuestion({ status: 'Answered' })} onAnswer={vi.fn()} />)
    expect(screen.getByText('Answered')).toBeTruthy()
  })

  it('shows "Expired" status badge', () => {
    render(<QuestionCard question={makeQuestion({ status: 'Expired' })} onAnswer={vi.fn()} />)
    expect(screen.getByText('Expired')).toBeTruthy()
  })

  it('shows Answer button for Pending questions', () => {
    render(<QuestionCard question={makeQuestion({ status: 'Pending' })} onAnswer={vi.fn()} />)
    expect(screen.getByText('Answer')).toBeTruthy()
  })

  it('does not show Answer button for Answered questions', () => {
    render(<QuestionCard question={makeQuestion({ status: 'Answered' })} onAnswer={vi.fn()} />)
    expect(screen.queryByText('Answer')).toBeNull()
  })

  it('does not show Answer button for Expired questions', () => {
    render(<QuestionCard question={makeQuestion({ status: 'Expired' })} onAnswer={vi.fn()} />)
    expect(screen.queryByText('Answer')).toBeNull()
  })

  it('calls onAnswer with the question when Answer is clicked', () => {
    const onAnswer = vi.fn()
    const question = makeQuestion()
    render(<QuestionCard question={question} onAnswer={onAnswer} />)
    fireEvent.click(screen.getByText('Answer'))
    expect(onAnswer).toHaveBeenCalledWith(question)
  })

  it('shows submitted answer text', () => {
    render(
      <QuestionCard
        question={makeQuestion({ status: 'Answered', answer: 'yes' })}
        onAnswer={vi.fn()}
      />,
    )
    expect(screen.getByText('Answer submitted')).toBeTruthy()
    expect(screen.getByText('yes')).toBeTruthy()
  })
})
