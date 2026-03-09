/**
 * Tests for AnswerDialog component.
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { AnswerDialog } from '@/components/questions/AnswerDialog'
import type { QuestionInfo } from '@/types/ask'

const QUESTION: QuestionInfo = {
  question_id: 'q-1',
  notification_id: 'notif-abc',
  check_type: 'TmuxSessions',
  asked_at: '2024-06-01T12:00:00Z',
  status: 'Pending',
}

describe('AnswerDialog', () => {
  it('does not render when open=false', () => {
    render(
      <AnswerDialog
        open={false}
        question={QUESTION}
        answering={false}
        onSubmit={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('renders dialog when open=true', () => {
    render(
      <AnswerDialog
        open
        question={QUESTION}
        answering={false}
        onSubmit={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    expect(screen.getByRole('dialog')).toBeTruthy()
  })

  it('shows "Answer Question" title', () => {
    render(
      <AnswerDialog open question={QUESTION} answering={false} onSubmit={vi.fn()} onClose={vi.fn()} />,
    )
    expect(screen.getByText('Answer Question')).toBeTruthy()
  })

  it('shows notification ID in context', () => {
    render(
      <AnswerDialog open question={QUESTION} answering={false} onSubmit={vi.fn()} onClose={vi.fn()} />,
    )
    expect(screen.getByText('notif-abc')).toBeTruthy()
  })

  it('shows quick answer buttons', () => {
    render(
      <AnswerDialog open question={QUESTION} answering={false} onSubmit={vi.fn()} onClose={vi.fn()} />,
    )
    expect(screen.getByText('Yes, start sessions')).toBeTruthy()
    expect(screen.getByText('No, ignore for now')).toBeTruthy()
    expect(screen.getByText('Acknowledged')).toBeTruthy()
  })

  it('calls onSubmit with quick answer value when clicked', () => {
    const onSubmit = vi.fn()
    render(
      <AnswerDialog open question={QUESTION} answering={false} onSubmit={onSubmit} onClose={vi.fn()} />,
    )
    fireEvent.click(screen.getByText('Yes, start sessions'))
    expect(onSubmit).toHaveBeenCalledWith('q-1', 'yes')
  })

  it('enables submit button only when textarea has content', () => {
    render(
      <AnswerDialog open question={QUESTION} answering={false} onSubmit={vi.fn()} onClose={vi.fn()} />,
    )
    const submitBtn = screen.getByText('Submit Answer')
    expect((submitBtn as HTMLButtonElement).disabled).toBe(true)

    fireEvent.change(screen.getByLabelText('Custom answer'), { target: { value: 'my answer' } })
    expect((submitBtn as HTMLButtonElement).disabled).toBe(false)
  })

  it('calls onSubmit with typed answer on form submit', () => {
    const onSubmit = vi.fn()
    render(
      <AnswerDialog open question={QUESTION} answering={false} onSubmit={onSubmit} onClose={vi.fn()} />,
    )
    fireEvent.change(screen.getByLabelText('Custom answer'), { target: { value: 'my answer' } })
    fireEvent.click(screen.getByText('Submit Answer'))
    expect(onSubmit).toHaveBeenCalledWith('q-1', 'my answer')
  })

  it('calls onClose when Cancel is clicked', () => {
    const onClose = vi.fn()
    render(
      <AnswerDialog open question={QUESTION} answering={false} onSubmit={vi.fn()} onClose={onClose} />,
    )
    fireEvent.click(screen.getByText('Cancel'))
    expect(onClose).toHaveBeenCalledOnce()
  })

  it('closes when close button (×) is clicked', () => {
    const onClose = vi.fn()
    render(
      <AnswerDialog open question={QUESTION} answering={false} onSubmit={vi.fn()} onClose={onClose} />,
    )
    fireEvent.click(screen.getByLabelText('Close answer dialog'))
    expect(onClose).toHaveBeenCalledOnce()
  })

  it('shows error when answerError is set', () => {
    render(
      <AnswerDialog
        open
        question={QUESTION}
        answering={false}
        answerError="Network error"
        onSubmit={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    expect(screen.getByText('Network error')).toBeTruthy()
  })

  it('shows "Submitting…" when answering=true', () => {
    render(
      <AnswerDialog open question={QUESTION} answering onSubmit={vi.fn()} onClose={vi.fn()} />,
    )
    expect(screen.getByText('Submitting…')).toBeTruthy()
  })
})
