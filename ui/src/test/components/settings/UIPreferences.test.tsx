import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { UIPreferences } from '@/components/settings/UIPreferences'
import type { Settings } from '@/stores/settingsStore'

const defaultUI: Settings['ui'] = {
  theme: 'system',
  sidebarDefaultOpen: true,
  refreshInterval: 30,
  notificationsEnabled: true,
  logViewLines: 100,
}

describe('UIPreferences', () => {
  it('renders all preference fields', () => {
    render(<UIPreferences ui={defaultUI} onSave={vi.fn()} />)
    expect(screen.getByLabelText(/theme/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/open by default/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/refresh interval/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/enable desktop notifications/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/log view lines/i)).toBeInTheDocument()
  })

  it('calls onSave when Save is clicked with current values', () => {
    const onSave = vi.fn()
    render(<UIPreferences ui={defaultUI} onSave={onSave} />)

    fireEvent.click(screen.getByRole('button', { name: /^save$/i }))

    expect(onSave).toHaveBeenCalledWith(defaultUI)
  })

  it('changing theme select updates the value passed to onSave', () => {
    const onSave = vi.fn()
    render(<UIPreferences ui={defaultUI} onSave={onSave} />)

    const themeSelect = screen.getByLabelText(/theme/i)
    fireEvent.change(themeSelect, { target: { value: 'dark' } })

    fireEvent.click(screen.getByRole('button', { name: /^save$/i }))

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ theme: 'dark' }),
    )
  })

  it('unchecking sidebar checkbox updates the value passed to onSave', () => {
    const onSave = vi.fn()
    render(<UIPreferences ui={defaultUI} onSave={onSave} />)

    const sidebarCheckbox = screen.getByLabelText(/open by default/i)
    fireEvent.click(sidebarCheckbox)

    fireEvent.click(screen.getByRole('button', { name: /^save$/i }))

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ sidebarDefaultOpen: false }),
    )
  })

  it('changing refresh interval updates the value passed to onSave', () => {
    const onSave = vi.fn()
    render(<UIPreferences ui={defaultUI} onSave={onSave} />)

    const intervalSelect = screen.getByLabelText(/refresh interval/i)
    fireEvent.change(intervalSelect, { target: { value: '120' } })

    fireEvent.click(screen.getByRole('button', { name: /^save$/i }))

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ refreshInterval: 120 }),
    )
  })
})
