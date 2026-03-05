import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import App from '../App'

describe('App', () => {
  it('renders the app root without crashing', () => {
    render(<App />)
    expect(document.getElementById('root') ?? document.body).toBeTruthy()
  })

  it('renders the agentd heading', () => {
    render(<App />)
    expect(screen.getByRole('heading', { name: /agentd/i })).toBeInTheDocument()
  })

  it('renders the coming soon message', () => {
    render(<App />)
    expect(screen.getByText(/coming soon/i)).toBeInTheDocument()
  })
})
