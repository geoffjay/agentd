import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { AboutSection } from '@/components/settings/AboutSection'

describe('AboutSection', () => {
  it('renders app version', () => {
    render(<AboutSection />)
    // VITE_APP_VERSION is not set in tests, so falls back to '0.1.0'
    expect(screen.getByText('0.1.0')).toBeInTheDocument()
  })

  it('renders GitHub link', () => {
    render(<AboutSection />)
    const githubLink = screen.getByRole('link', { name: /github/i })
    expect(githubLink).toBeInTheDocument()
    expect(githubLink).toHaveAttribute('href', expect.stringContaining('github.com'))
  })

  it('renders documentation link', () => {
    render(<AboutSection />)
    const docsLink = screen.getByRole('link', { name: /documentation/i })
    expect(docsLink).toBeInTheDocument()
  })
})
