/**
 * Tests for AgentTable usage columns (Cost, Tokens, Cache Hit).
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import type { ReactNode } from 'react'
import { AgentTable } from '@/components/agents/AgentTable'
import { makeAgentList, resetAgentSeq } from '@/test/mocks/factories'
import type { SortField, SortDir } from '@/hooks/useAgents'
import type { AgentUsageStats } from '@/types/orchestrator'

function wrapper({ children }: { children: ReactNode }) {
  return <MemoryRouter>{children}</MemoryRouter>
}

function makeUsageStats(overrides?: Partial<AgentUsageStats['cumulative']>): AgentUsageStats {
  return {
    agent_id: '',
    cumulative: {
      input_tokens: 1000,
      output_tokens: 500,
      cache_read_input_tokens: 200,
      cache_creation_input_tokens: 50,
      total_cost_usd: 0.15,
      num_turns: 5,
      duration_ms: 3000,
      duration_api_ms: 2500,
      result_count: 5,
      started_at: '2024-01-01T00:00:00Z',
      ...overrides,
    },
    session_count: 1,
  }
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

describe('AgentTable usage columns', () => {
  beforeEach(() => {
    resetAgentSeq()
  })

  it('renders Cost column header with sort button', () => {
    const agents = makeAgentList(1)
    render(<AgentTable {...defaultProps} agents={agents} />, { wrapper })
    expect(screen.getByRole('button', { name: /cost/i })).toBeInTheDocument()
  })

  it('renders Tokens column header with sort button', () => {
    const agents = makeAgentList(1)
    render(<AgentTable {...defaultProps} agents={agents} />, { wrapper })
    expect(screen.getByRole('button', { name: /tokens/i })).toBeInTheDocument()
  })

  it('renders Cache Hit column header with sort button', () => {
    const agents = makeAgentList(1)
    render(<AgentTable {...defaultProps} agents={agents} />, { wrapper })
    expect(screen.getByRole('button', { name: /cache hit/i })).toBeInTheDocument()
  })

  it('calls onSort with "cost" when Cost header is clicked', () => {
    const onSort = vi.fn()
    const agents = makeAgentList(1)
    render(<AgentTable {...defaultProps} agents={agents} onSort={onSort} />, { wrapper })
    fireEvent.click(screen.getByRole('button', { name: /cost/i }))
    expect(onSort).toHaveBeenCalledWith('cost')
  })

  it('calls onSort with "tokens" when Tokens header is clicked', () => {
    const onSort = vi.fn()
    const agents = makeAgentList(1)
    render(<AgentTable {...defaultProps} agents={agents} onSort={onSort} />, { wrapper })
    fireEvent.click(screen.getByRole('button', { name: /tokens/i }))
    expect(onSort).toHaveBeenCalledWith('tokens')
  })

  it('calls onSort with "cache" when Cache Hit header is clicked', () => {
    const onSort = vi.fn()
    const agents = makeAgentList(1)
    render(<AgentTable {...defaultProps} agents={agents} onSort={onSort} />, { wrapper })
    fireEvent.click(screen.getByRole('button', { name: /cache hit/i }))
    expect(onSort).toHaveBeenCalledWith('cache')
  })

  it('displays formatted cost when usageMap is provided', () => {
    const agents = makeAgentList(1)
    const usageMap = new Map<string, AgentUsageStats>()
    usageMap.set(agents[0].id, makeUsageStats({ total_cost_usd: 1.23 }))
    render(<AgentTable {...defaultProps} agents={agents} usageMap={usageMap} />, { wrapper })
    expect(screen.getByText('$1.23')).toBeInTheDocument()
  })

  it('displays formatted tokens when usageMap is provided', () => {
    const agents = makeAgentList(1)
    const usageMap = new Map<string, AgentUsageStats>()
    usageMap.set(agents[0].id, makeUsageStats({ input_tokens: 15000, output_tokens: 5000 }))
    render(<AgentTable {...defaultProps} agents={agents} usageMap={usageMap} />, { wrapper })
    expect(screen.getByText('20.0k')).toBeInTheDocument()
  })

  it('displays cache hit percentage when usageMap is provided', () => {
    const agents = makeAgentList(1)
    const usageMap = new Map<string, AgentUsageStats>()
    // cache_read=200, cache_creation=50, input=1000 → 200/1250 = 16%
    usageMap.set(agents[0].id, makeUsageStats())
    render(<AgentTable {...defaultProps} agents={agents} usageMap={usageMap} />, { wrapper })
    expect(screen.getByText('16%')).toBeInTheDocument()
  })

  it('shows dash when usageMap is not provided', () => {
    const agents = makeAgentList(1)
    render(<AgentTable {...defaultProps} agents={agents} />, { wrapper })
    // Without usageMap, the cost/tokens/cache cells should show dashes
    const dashes = screen.getAllByText('—')
    expect(dashes.length).toBeGreaterThanOrEqual(3)
  })

  it('shows dash for agents not in usageMap', () => {
    const agents = makeAgentList(2)
    const usageMap = new Map<string, AgentUsageStats>()
    // Only provide usage for first agent
    usageMap.set(agents[0].id, makeUsageStats({ total_cost_usd: 5.0 }))
    render(<AgentTable {...defaultProps} agents={agents} usageMap={usageMap} />, { wrapper })
    expect(screen.getByText('$5.00')).toBeInTheDocument()
    // Second agent should have dashes
    const dashes = screen.getAllByText('—')
    expect(dashes.length).toBeGreaterThanOrEqual(3)
  })
})
