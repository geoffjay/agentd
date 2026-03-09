import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { ServiceConfig } from '@/components/settings/ServiceConfig'
import type { Settings } from '@/stores/settingsStore'

const defaultServices: Settings['services'] = {
  orchestratorUrl: 'http://localhost:17006',
  notifyUrl: 'http://localhost:17004',
  askUrl: 'http://localhost:17001',
}

describe('ServiceConfig', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  it('renders three service rows', () => {
    render(<ServiceConfig services={defaultServices} onSave={vi.fn()} />)
    const inputs = screen.getAllByRole('textbox')
    expect(inputs).toHaveLength(3)
  })

  it('shows service names: Orchestrator, Notify, Ask', () => {
    render(<ServiceConfig services={defaultServices} onSave={vi.fn()} />)
    expect(screen.getByText('Orchestrator')).toBeInTheDocument()
    expect(screen.getByText('Notify')).toBeInTheDocument()
    expect(screen.getByText('Ask')).toBeInTheDocument()
  })

  it('calls onSave with updated URL when Save is clicked', () => {
    const onSave = vi.fn()
    render(<ServiceConfig services={defaultServices} onSave={onSave} />)

    const orchestratorInput = screen.getByRole('textbox', {
      name: /orchestrator/i,
    }) as HTMLInputElement
    fireEvent.change(orchestratorInput, { target: { value: 'http://localhost:9999' } })

    fireEvent.click(screen.getByRole('button', { name: /^save$/i }))

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ orchestratorUrl: 'http://localhost:9999' }),
    )
  })

  it('shows loading state when testing', async () => {
    // Mock fetch to never resolve during this test
    vi.stubGlobal(
      'fetch',
      vi.fn(
        () =>
          new Promise<Response>(() => {
            // intentionally never resolves
          }),
      ),
    )

    render(<ServiceConfig services={defaultServices} onSave={vi.fn()} />)

    const testButtons = screen.getAllByRole('button', { name: /test/i })
    fireEvent.click(testButtons[0])

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /test orchestrator/i })).toBeDisabled()
    })
  })

  it('shows success indicator after successful health test', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true } as Response))

    render(<ServiceConfig services={defaultServices} onSave={vi.fn()} />)

    const testButtons = screen.getAllByRole('button', { name: /test/i })
    fireEvent.click(testButtons[0])

    await waitFor(() => {
      expect(screen.getByText('✓')).toBeInTheDocument()
    })
  })

  it('shows error indicator after failed health test', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('Network error')))

    render(<ServiceConfig services={defaultServices} onSave={vi.fn()} />)

    const testButtons = screen.getAllByRole('button', { name: /test/i })
    fireEvent.click(testButtons[0])

    await waitFor(() => {
      expect(screen.getByText('✗')).toBeInTheDocument()
    })
  })
})
