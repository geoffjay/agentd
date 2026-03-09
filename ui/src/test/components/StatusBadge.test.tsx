import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { StatusBadge } from '@/components/common/StatusBadge'

describe('StatusBadge', () => {
  describe('badge variant (default)', () => {
    it('renders agent status Running', () => {
      render(<StatusBadge status="Running" />)
      expect(screen.getByRole('status')).toHaveTextContent('Running')
    })

    it('renders agent status Failed', () => {
      render(<StatusBadge status="Failed" />)
      const badge = screen.getByRole('status')
      expect(badge).toHaveTextContent('Failed')
      expect(badge.className).toContain('red')
    })

    it('renders service status healthy', () => {
      render(<StatusBadge status="healthy" />)
      const badge = screen.getByRole('status')
      expect(badge).toHaveTextContent('healthy')
      expect(badge.className).toContain('green')
    })

    it('renders service status down with red colour', () => {
      render(<StatusBadge status="down" />)
      const badge = screen.getByRole('status')
      expect(badge.className).toContain('red')
    })

    it('renders notification status Pending', () => {
      render(<StatusBadge status="Pending" />)
      expect(screen.getByRole('status')).toHaveTextContent('Pending')
    })
  })

  describe('dot variant', () => {
    it('renders a coloured dot with aria-label', () => {
      render(<StatusBadge status="Running" variant="dot" />)
      const dot = screen.getByRole('status', { name: 'Running' })
      expect(dot.className).toContain('rounded-full')
      expect(dot.className).toContain('green')
    })

    it('applies correct colour for Failed', () => {
      render(<StatusBadge status="Failed" variant="dot" />)
      const dot = screen.getByRole('status', { name: 'Failed' })
      expect(dot.className).toContain('red')
    })

    it('applies custom className', () => {
      render(<StatusBadge status="Running" variant="dot" className="ml-2" />)
      expect(screen.getByRole('status').className).toContain('ml-2')
    })
  })
})
