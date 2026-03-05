import { describe, it, expect } from 'vitest'
import { render } from '@testing-library/react'
import {
  Skeleton,
  CardSkeleton,
  ListItemSkeleton,
  ChartSkeleton,
} from '@/components/common/LoadingSkeleton'

describe('LoadingSkeleton', () => {
  describe('Skeleton', () => {
    it('renders an animated pulse element', () => {
      const { container } = render(<Skeleton />)
      expect(container.firstChild).toHaveClass('animate-pulse')
    })

    it('applies custom className', () => {
      const { container } = render(<Skeleton className="h-4 w-32" />)
      expect(container.firstChild).toHaveClass('h-4', 'w-32')
    })
  })

  describe('CardSkeleton', () => {
    it('renders with aria-busy="true"', () => {
      const { container } = render(<CardSkeleton />)
      expect(container.querySelector('[aria-busy="true"]')).toBeTruthy()
    })

    it('renders with aria-label "Loading…"', () => {
      const { container } = render(<CardSkeleton />)
      const el = container.querySelector('[aria-label="Loading…"]')
      expect(el).toBeTruthy()
    })
  })

  describe('ListItemSkeleton', () => {
    it('renders the default number of rows (3)', () => {
      const { container } = render(<ListItemSkeleton />)
      // Each row has a rounded-full skeleton for the avatar
      const avatars = container.querySelectorAll('.rounded-full')
      expect(avatars.length).toBeGreaterThanOrEqual(3)
    })

    it('renders a custom number of rows', () => {
      const { container } = render(<ListItemSkeleton rows={5} />)
      const avatars = container.querySelectorAll('.rounded-full')
      expect(avatars.length).toBeGreaterThanOrEqual(5)
    })
  })

  describe('ChartSkeleton', () => {
    it('renders with the default height', () => {
      const { container } = render(<ChartSkeleton />)
      const wrapper = container.firstChild as HTMLElement
      expect(wrapper.style.height).toBe('160px')
    })

    it('renders with a custom height', () => {
      const { container } = render(<ChartSkeleton height={240} />)
      const wrapper = container.firstChild as HTMLElement
      expect(wrapper.style.height).toBe('240px')
    })
  })
})
