import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import type { ReactNode } from 'react'
import { UIPreferences } from '@/components/settings/UIPreferences'
import { ThemeProvider } from '@/hooks/useTheme'
import type { Settings } from '@/stores/settingsStore'

// UIPreferences now calls useTheme() → must be wrapped in ThemeProvider
function wrapper({ children }: { children: ReactNode }) {
  return <ThemeProvider>{children}</ThemeProvider>
}

const defaultUI: Settings['ui'] = {
  theme: 'system',
  sidebarDefaultOpen: true,
  refreshInterval: 30,
  notificationsEnabled: true,
  logViewLines: 100,
}

describe('UIPreferences', () => {
  it('renders all preference fields', () => {
    render(<UIPreferences ui={defaultUI} onSave={vi.fn()} />, { wrapper })
    expect(screen.getByLabelText(/theme/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/open by default/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/refresh interval/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/enable desktop notifications/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/log view lines/i)).toBeInTheDocument()
  })

  it('calls onSave when Save is clicked with current values', () => {
    const onSave = vi.fn()
    render(<UIPreferences ui={defaultUI} onSave={onSave} />, { wrapper })

    fireEvent.click(screen.getByRole('button', { name: /^save$/i }))

    expect(onSave).toHaveBeenCalledWith(defaultUI)
  })

  it('changing theme select updates the value passed to onSave', () => {
    const onSave = vi.fn()
    render(<UIPreferences ui={defaultUI} onSave={onSave} />, { wrapper })

    const themeSelect = screen.getByLabelText(/theme/i)
    fireEvent.change(themeSelect, { target: { value: 'dark' } })

    fireEvent.click(screen.getByRole('button', { name: /^save$/i }))

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ theme: 'dark' }),
    )
  })

  it('changing theme select applies theme immediately to DOM', () => {
    render(<UIPreferences ui={defaultUI} onSave={vi.fn()} />, { wrapper })

    const themeSelect = screen.getByLabelText(/theme/i)
    fireEvent.change(themeSelect, { target: { value: 'dark' } })

    expect(document.documentElement.classList.contains('dark')).toBe(true)
  })

  it('unchecking sidebar checkbox updates the value passed to onSave', () => {
    const onSave = vi.fn()
    render(<UIPreferences ui={defaultUI} onSave={onSave} />, { wrapper })

    const sidebarCheckbox = screen.getByLabelText(/open by default/i)
    fireEvent.click(sidebarCheckbox)

    fireEvent.click(screen.getByRole('button', { name: /^save$/i }))

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ sidebarDefaultOpen: false }),
    )
  })

  it('changing refresh interval updates the value passed to onSave', () => {
    const onSave = vi.fn()
    render(<UIPreferences ui={defaultUI} onSave={onSave} />, { wrapper })

    const intervalSelect = screen.getByLabelText(/refresh interval/i)
    fireEvent.change(intervalSelect, { target: { value: '120' } })

    fireEvent.click(screen.getByRole('button', { name: /^save$/i }))

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ refreshInterval: 120 }),
    )
  })
})
