import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { AgentPolicyEditor } from '@/components/agents/AgentPolicyEditor'

describe('AgentPolicyEditor', () => {
  it('renders policy type dropdown', () => {
    render(
      <AgentPolicyEditor
        policy={{ type: 'AllowAll' }}
        onSave={vi.fn()}
      />,
    )
    expect(screen.getByRole('combobox', { name: /policy type/i })).toBeInTheDocument()
  })

  it('shows AllowAll selected by default when policy is AllowAll', () => {
    render(
      <AgentPolicyEditor
        policy={{ type: 'AllowAll' }}
        onSave={vi.fn()}
      />,
    )
    const select = screen.getByRole('combobox', { name: /policy type/i })
    expect(select).toHaveValue('AllowAll')
  })

  it('does not show tool list for AllowAll', () => {
    render(
      <AgentPolicyEditor
        policy={{ type: 'AllowAll' }}
        onSave={vi.fn()}
      />,
    )
    expect(screen.queryByRole('textbox', { name: /tool names/i })).not.toBeInTheDocument()
  })

  it('shows tool list input when AllowList is selected', () => {
    render(
      <AgentPolicyEditor
        policy={{ type: 'AllowList', tools: ['bash'] }}
        onSave={vi.fn()}
      />,
    )
    expect(screen.getByRole('textbox', { name: /tool names/i })).toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: /tool names/i })).toHaveValue('bash')
  })

  it('shows tool list input when DenyList is selected', () => {
    render(
      <AgentPolicyEditor
        policy={{ type: 'DenyList', tools: ['rm', 'dd'] }}
        onSave={vi.fn()}
      />,
    )
    expect(screen.getByRole('textbox', { name: /tool names/i })).toHaveValue('rm, dd')
  })

  it('calls onSave with correct AllowAll policy', async () => {
    const onSave = vi.fn().mockResolvedValue(undefined)
    render(
      <AgentPolicyEditor
        policy={{ type: 'AllowAll' }}
        onSave={onSave}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: /save policy/i }))
    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith({ type: 'AllowAll' }),
    )
  })

  it('calls onSave with parsed tools for AllowList', async () => {
    const onSave = vi.fn().mockResolvedValue(undefined)
    render(
      <AgentPolicyEditor
        policy={{ type: 'AllowList', tools: [] }}
        onSave={onSave}
      />,
    )
    // Change tools
    fireEvent.change(screen.getByRole('textbox', { name: /tool names/i }), {
      target: { value: 'bash, read_file, write_file' },
    })
    fireEvent.click(screen.getByRole('button', { name: /save policy/i }))
    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith({
        type: 'AllowList',
        tools: ['bash', 'read_file', 'write_file'],
      }),
    )
  })

  it('shows error when onSave throws', async () => {
    const onSave = vi.fn().mockRejectedValue(new Error('Save failed'))
    render(
      <AgentPolicyEditor
        policy={{ type: 'AllowAll' }}
        onSave={onSave}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: /save policy/i }))
    await waitFor(() =>
      expect(screen.getByRole('alert')).toHaveTextContent('Save failed'),
    )
  })

  it('shows "Saving…" label when saving=true', () => {
    render(
      <AgentPolicyEditor
        policy={{ type: 'AllowAll' }}
        saving
        onSave={vi.fn()}
      />,
    )
    expect(screen.getByRole('button', { name: /saving/i })).toBeInTheDocument()
  })
})
