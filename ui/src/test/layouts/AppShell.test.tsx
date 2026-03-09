import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, act } from '@testing-library/react'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { AppShell } from '@/layouts/AppShell'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const STORAGE_KEY = 'agentd:sidebar:open'

function renderAppShell(initialPath = '/') {
  return render(
    <MemoryRouter initialEntries={[initialPath]}>
      <Routes>
        <Route element={<AppShell />}>
          <Route index element={<div>Dashboard content</div>} />
          <Route path="/agents" element={<div>Agents content</div>} />
          <Route path="/notifications" element={<div>Notifications content</div>} />
          <Route path="/settings" element={<div>Settings content</div>} />
        </Route>
      </Routes>
    </MemoryRouter>,
  )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('AppShell', () => {
  beforeEach(() => {
    localStorage.clear()
    vi.stubGlobal('innerWidth', 1280) // simulate desktop
  })

  afterEach(() => {
    localStorage.clear()
    vi.unstubAllGlobals()
  })

  it('renders the header, sidebar, and content', () => {
    renderAppShell()
    expect(screen.getByRole('banner')).toBeInTheDocument() // <header>
    expect(screen.getByRole('complementary')).toBeInTheDocument() // <aside>
    expect(screen.getByRole('main')).toBeInTheDocument()
  })

  it('renders child route content via Outlet', () => {
    renderAppShell()
    expect(screen.getByText('Dashboard content')).toBeInTheDocument()
  })

  it('sidebar is open by default when no localStorage value exists', () => {
    localStorage.removeItem(STORAGE_KEY)
    renderAppShell()
    // The sidebar aside should have w-60 when open
    const sidebar = screen.getByRole('complementary')
    expect(sidebar.className).toContain('w-60')
  })

  it('restores sidebar state from localStorage', () => {
    localStorage.setItem(STORAGE_KEY, 'false')
    renderAppShell()
    const sidebar = screen.getByRole('complementary')
    expect(sidebar.className).toContain('w-16')
  })

  it('toggles sidebar when the hamburger button is clicked', () => {
    renderAppShell()
    const toggle = screen.getByRole('button', { name: /toggle sidebar/i })

    // Initial state: open (w-60)
    const sidebar = screen.getByRole('complementary')
    expect(sidebar.className).toContain('w-60')

    // Click to close
    fireEvent.click(toggle)
    expect(sidebar.className).toContain('w-16')

    // Click to re-open
    fireEvent.click(toggle)
    expect(sidebar.className).toContain('w-60')
  })

  it('persists sidebar state to localStorage on toggle', () => {
    renderAppShell()
    const toggle = screen.getByRole('button', { name: /toggle sidebar/i })
    fireEvent.click(toggle)
    expect(localStorage.getItem(STORAGE_KEY)).toBe('false')
    fireEvent.click(toggle)
    expect(localStorage.getItem(STORAGE_KEY)).toBe('true')
  })

  it('toggles sidebar with Ctrl+B keyboard shortcut', () => {
    renderAppShell()
    const sidebar = screen.getByRole('complementary')
    expect(sidebar.className).toContain('w-60')

    act(() => {
      fireEvent.keyDown(document, { key: 'b', ctrlKey: true })
    })
    expect(sidebar.className).toContain('w-16')

    act(() => {
      fireEvent.keyDown(document, { key: 'b', ctrlKey: true })
    })
    expect(sidebar.className).toContain('w-60')
  })
})
