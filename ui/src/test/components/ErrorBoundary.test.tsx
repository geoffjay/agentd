/**
 * Tests for ErrorBoundary component.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { ErrorBoundary } from '@/components/common/ErrorBoundary'

// Suppress console.error for expected error boundary output
const originalError = console.error

beforeEach(() => {
  console.error = vi.fn()
})

afterEach(() => {
  console.error = originalError
})

// Component that throws on demand
function Bomb({ shouldThrow }: { shouldThrow: boolean }) {
  if (shouldThrow) {
    throw new Error('Test explosion')
  }
  return <div>All good</div>
}

describe('ErrorBoundary', () => {
  it('renders children when no error', () => {
    render(
      <ErrorBoundary>
        <Bomb shouldThrow={false} />
      </ErrorBoundary>,
    )
    expect(screen.getByText('All good')).toBeInTheDocument()
  })

  it('shows fallback UI when a child throws', () => {
    render(
      <ErrorBoundary>
        <Bomb shouldThrow />
      </ErrorBoundary>,
    )
    expect(screen.getByRole('alert')).toBeInTheDocument()
    expect(screen.getByText(/Something went wrong/i)).toBeInTheDocument()
  })

  it('shows "Try Again" and "Go Home" buttons in fallback', () => {
    render(
      <ErrorBoundary>
        <Bomb shouldThrow />
      </ErrorBoundary>,
    )
    expect(screen.getByRole('button', { name: /try again/i })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /go home/i })).toBeInTheDocument()
  })

  it('resets error state when "Try Again" is clicked', () => {
    const onReset = vi.fn()
    render(
      <ErrorBoundary onReset={onReset}>
        <Bomb shouldThrow />
      </ErrorBoundary>,
    )
    fireEvent.click(screen.getByRole('button', { name: /try again/i }))
    expect(onReset).toHaveBeenCalledTimes(1)
  })

  it('shows error details when details toggle is clicked', () => {
    render(
      <ErrorBoundary>
        <Bomb shouldThrow />
      </ErrorBoundary>,
    )
    const toggle = screen.getByRole('button', { name: /show error details/i })
    fireEvent.click(toggle)
    // The details section should now be visible (stack trace appears)
    expect(screen.getByRole('button', { name: /hide error details/i })).toBeInTheDocument()
  })

  it('calls custom fallback render prop when provided', () => {
    const customFallback = vi.fn((err: Error) => <div>Custom: {err.message}</div>)
    render(
      <ErrorBoundary fallback={customFallback}>
        <Bomb shouldThrow />
      </ErrorBoundary>,
    )
    expect(customFallback).toHaveBeenCalled()
    expect(screen.getByText(/Custom: Test explosion/)).toBeInTheDocument()
  })

  it('uses page level styles when level="page"', () => {
    render(
      <ErrorBoundary level="page">
        <Bomb shouldThrow />
      </ErrorBoundary>,
    )
    expect(screen.getByText(/This page crashed/i)).toBeInTheDocument()
  })

  it('logs error to console', () => {
    render(
      <ErrorBoundary>
        <Bomb shouldThrow />
      </ErrorBoundary>,
    )
    expect(console.error).toHaveBeenCalled()
  })
})
