import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import App from '../App'

describe('App', () => {
  it('renders the app root without crashing', () => {
    render(<App />)
    expect(document.getElementById('root') ?? document.body).toBeTruthy()
  })

  it('renders the header with the agentd logo link', () => {
    render(<App />)
    expect(screen.getByRole('link', { name: /agentd home/i })).toBeInTheDocument()
  })

  it('renders the sidebar navigation', () => {
    render(<App />)
    expect(screen.getByRole('navigation')).toBeInTheDocument()
  })

  it('renders the main content area', () => {
    render(<App />)
    expect(screen.getByRole('main')).toBeInTheDocument()
  })

  it('renders the dashboard page by default', () => {
    render(<App />)
    expect(screen.getByRole('heading', { name: /dashboard/i })).toBeInTheDocument()
  })
})
