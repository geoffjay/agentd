import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import type { ReactNode } from 'react'
import { AgentTable } from '@/components/agents/AgentTable'
import { makeAgentList, resetAgentSeq } from '@/test/mocks/factories'
import type { SortField, SortDir } from '@/hooks/useAgents'

function wrapper({ children }: { children: ReactNode }) {
  return <MemoryRouter>{children}</MemoryRouter>
}

const defaultProps = {
  agents: [],
  loading: false,
  sortBy: 'created_at' as SortField,
  sortDir: 'desc' as SortDir,
  onSort: vi.fn(),
  onDelete: vi.fn(),
  onBulkDelete: vi.fn(),
  selectedIds: [],
  onSelectChange: vi.fn(),
}

describe('AgentTable', () => {
  beforeEach(() => {
    resetAgentSeq()
  })

  it('renders empty state when no agents', () => {
    render(<AgentTable {...defaultProps} />, { wrapper })
    expect(screen.getByText(/no agents found/i)).toBeInTheDocument()
  })

  it('renders loading skeletons when loading', () => {
    render(<AgentTable {...defaultProps} loading />, { wrapper })
    // Loading state renders a skeleton in place of rows — the table itself should be present
    expect(screen.getByRole('table')).toBeInTheDocument()
  })

  it('renders agent rows', () => {
    const agents = makeAgentList(3)
    render(<AgentTable {...defaultProps} agents={agents} />, { wrapper })
    for (const agent of agents) {
      expect(screen.getByText(agent.name)).toBeInTheDocument()
    }
  })

  it('calls onSort when a column header is clicked', () => {
    const onSort = vi.fn()
    const agents = makeAgentList(2)
    render(<AgentTable {...defaultProps} agents={agents} onSort={onSort} />, { wrapper })
    fireEvent.click(screen.getByRole('button', { name: /name/i }))
    expect(onSort).toHaveBeenCalledWith('name')
  })

  it('renders select-all checkbox', () => {
    const agents = makeAgentList(2)
    render(<AgentTable {...defaultProps} agents={agents} />, { wrapper })
    expect(screen.getByRole('checkbox', { name: /select all/i })).toBeInTheDocument()
  })

  it('calls onSelectChange with all agent ids when select-all is checked', () => {
    const onSelectChange = vi.fn()
    const agents = makeAgentList(2)
    render(
      <AgentTable {...defaultProps} agents={agents} onSelectChange={onSelectChange} />,
      { wrapper },
    )
    fireEvent.click(screen.getByRole('checkbox', { name: /select all/i }))
    expect(onSelectChange).toHaveBeenCalledWith(agents.map(a => a.id))
  })

  it('shows bulk action toolbar when agents are selected', () => {
    const agents = makeAgentList(2)
    render(
      <AgentTable
        {...defaultProps}
        agents={agents}
        selectedIds={[agents[0].id]}
      />,
      { wrapper },
    )
    expect(screen.getByRole('button', { name: /terminate selected/i })).toBeInTheDocument()
  })

  it('does not show bulk toolbar when nothing is selected', () => {
    const agents = makeAgentList(2)
    render(<AgentTable {...defaultProps} agents={agents} selectedIds={[]} />, { wrapper })
    expect(screen.queryByRole('button', { name: /terminate selected/i })).not.toBeInTheDocument()
  })

  it('opens confirmation dialog when per-row terminate is clicked', async () => {
    const agents = makeAgentList(1)
    render(<AgentTable {...defaultProps} agents={agents} />, { wrapper })
    fireEvent.click(screen.getByRole('button', { name: /terminate agent/i }))
    await waitFor(() => {
      expect(screen.getByRole('alertdialog')).toBeInTheDocument()
    })
  })

  it('calls onDelete with correct id after confirming single terminate', async () => {
    const onDelete = vi.fn().mockResolvedValue(undefined)
    const agents = makeAgentList(1)
    render(
      <AgentTable {...defaultProps} agents={agents} onDelete={onDelete} />,
      { wrapper },
    )
    fireEvent.click(screen.getByRole('button', { name: /terminate agent/i }))
    await waitFor(() => screen.getByRole('alertdialog'))
    fireEvent.click(screen.getByRole('button', { name: /^terminate$/i }))
    await waitFor(() => {
      expect(onDelete).toHaveBeenCalledWith(agents[0].id)
    })
  })
})
