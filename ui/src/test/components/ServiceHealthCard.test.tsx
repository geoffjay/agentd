import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { ServiceHealthCard } from '@/components/dashboard/ServiceHealthCard'
import type { ServiceHealth } from '@/hooks/useServiceHealth'

// Mock useNavigate
const mockNavigate = vi.fn()
vi.mock('react-router-dom', async (importOriginal) => {
  const mod = await importOriginal()
  return {
    ...(mod as object),
    useNavigate: () => mockNavigate,
  }
})

const healthyService: ServiceHealth = {
  name: 'Orchestrator',
  key: 'orchestrator',
  port: 17006,
  status: 'healthy',
  version: '0.2.0',
  lastChecked: new Date(),
}

const downService: ServiceHealth = {
  name: 'Ask',
  key: 'ask',
  port: 17001,
  status: 'down',
  error: 'Service unreachable',
  lastChecked: new Date(),
}

function renderCard(service: ServiceHealth) {
  return render(
    <MemoryRouter>
      <ServiceHealthCard service={service} />
    </MemoryRouter>,
  )
}

describe('ServiceHealthCard', () => {
  it('renders service name', () => {
    renderCard(healthyService)
    expect(screen.getByText('Orchestrator')).toBeInTheDocument()
  })

  it('renders port number', () => {
    renderCard(healthyService)
    expect(screen.getByText(/Port 17006/)).toBeInTheDocument()
  })

  it('renders version when available', () => {
    renderCard(healthyService)
    expect(screen.getByText(/v0\.2\.0/)).toBeInTheDocument()
  })

  it('renders the healthy status badge', () => {
    renderCard(healthyService)
    expect(screen.getByRole('status')).toHaveTextContent('healthy')
  })

  it('renders error message when service is down', () => {
    renderCard(downService)
    expect(screen.getByText('Service unreachable')).toBeInTheDocument()
  })

  it('navigates to /agents when orchestrator card is clicked', () => {
    renderCard(healthyService)
    fireEvent.click(screen.getByRole('button'))
    expect(mockNavigate).toHaveBeenCalledWith('/agents')
  })

  it('navigates to /questions when ask card is clicked', () => {
    renderCard(downService)
    fireEvent.click(screen.getByRole('button'))
    expect(mockNavigate).toHaveBeenCalledWith('/questions')
  })

  it('is keyboard accessible via Enter key', () => {
    renderCard(healthyService)
    const card = screen.getByRole('button')
    fireEvent.keyDown(card, { key: 'Enter' })
    expect(mockNavigate).toHaveBeenCalled()
  })
})
