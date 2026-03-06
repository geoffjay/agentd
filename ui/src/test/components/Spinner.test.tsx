/**
 * Tests for Spinner component.
 */

import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { Spinner } from '@/components/common/Spinner'

describe('Spinner', () => {
  it('renders inline spinner with status role', () => {
    render(<Spinner />)
    expect(screen.getByRole('status', { name: 'Loading' })).toBeInTheDocument()
  })

  it('renders label text when provided', () => {
    render(<Spinner label="Please wait…" />)
    expect(screen.getByText('Please wait…')).toBeInTheDocument()
  })

  it('renders page variant with accessible label', () => {
    render(<Spinner variant="page" label="Loading app…" />)
    expect(screen.getByRole('status', { name: 'Loading app…' })).toBeInTheDocument()
  })

  it('renders overlay variant', () => {
    render(<Spinner variant="overlay" label="Saving" />)
    expect(screen.getByRole('status', { name: 'Saving' })).toBeInTheDocument()
  })

  it('applies correct size class for sm', () => {
    render(<Spinner size="sm" />)
    const spinner = screen.getByRole('status', { name: 'Loading' })
    expect(spinner.className).toContain('h-4')
  })

  it('applies correct size class for lg', () => {
    render(<Spinner size="lg" />)
    const spinner = screen.getByRole('status', { name: 'Loading' })
    expect(spinner.className).toContain('h-12')
  })
})
