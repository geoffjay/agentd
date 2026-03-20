/**
 * Tests for HumanIdentitySetup component.
 */

import { render, screen, fireEvent } from '@testing-library/react'
import { HumanIdentitySetup } from '@/components/communicate/HumanIdentitySetup'

describe('HumanIdentitySetup', () => {
  it('renders nothing when closed', () => {
    const { container } = render(
      <HumanIdentitySetup open={false} onSave={vi.fn()} />,
    )
    expect(container).toBeEmptyDOMElement()
  })

  it('renders the setup dialog when open', () => {
    render(<HumanIdentitySetup open={true} onSave={vi.fn()} />)
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(screen.getByText(/set up your identity/i)).toBeInTheDocument()
  })

  it('shows display name and identifier inputs', () => {
    render(<HumanIdentitySetup open={true} onSave={vi.fn()} />)
    expect(screen.getByPlaceholderText(/e\.g\. alice/i)).toBeInTheDocument()
    expect(screen.getByPlaceholderText(/e\.g\. human-alice/i)).toBeInTheDocument()
  })

  it('calls onSave with identifier and displayName', () => {
    const onSave = vi.fn()
    render(<HumanIdentitySetup open={true} onSave={onSave} />)

    fireEvent.change(screen.getByPlaceholderText(/e\.g\. alice/i), {
      target: { value: 'Alice' },
    })
    const identifierInput = screen.getByPlaceholderText(/e\.g\. human-alice/i)
    fireEvent.change(identifierInput, { target: { value: 'human-alice' } })

    fireEvent.click(screen.getByRole('button', { name: /get started/i }))
    expect(onSave).toHaveBeenCalledWith('human-alice', 'Alice')
  })

  it('shows validation errors when fields are empty', () => {
    render(<HumanIdentitySetup open={true} onSave={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: /get started/i }))
    expect(screen.getByText(/display name is required/i)).toBeInTheDocument()
  })

  it('shows error for invalid identifier format', () => {
    render(<HumanIdentitySetup open={true} onSave={vi.fn()} />)
    fireEvent.change(screen.getByPlaceholderText(/e\.g\. alice/i), {
      target: { value: 'Alice' },
    })
    fireEvent.change(screen.getByPlaceholderText(/e\.g\. human-alice/i), {
      target: { value: 'INVALID identifier!' },
    })
    fireEvent.click(screen.getByRole('button', { name: /get started/i }))
    expect(screen.getByText(/lowercase letters/i)).toBeInTheDocument()
  })

  it('submits on Enter key press', () => {
    const onSave = vi.fn()
    render(<HumanIdentitySetup open={true} onSave={onSave} />)

    fireEvent.change(screen.getByPlaceholderText(/e\.g\. alice/i), {
      target: { value: 'Bob' },
    })
    const identifierInput = screen.getByPlaceholderText(/e\.g\. human-alice/i)
    fireEvent.change(identifierInput, { target: { value: 'human-bob' } })
    fireEvent.keyDown(identifierInput.parentElement!, { key: 'Enter' })

    expect(onSave).toHaveBeenCalledWith('human-bob', 'Bob')
  })
})
