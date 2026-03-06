import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { AgentConfigPanel } from '@/components/agents/AgentConfigPanel'
import { makeAgent } from '@/test/mocks/factories'

describe('AgentConfigPanel', () => {
  it('renders configuration section', () => {
    const agent = makeAgent()
    render(<AgentConfigPanel agent={agent} />)
    expect(screen.getByRole('region', { name: /agent configuration/i })).toBeInTheDocument()
  })

  it('shows working directory and shell', () => {
    const agent = makeAgent()
    render(<AgentConfigPanel agent={agent} />)
    expect(screen.getByText(agent.config.working_dir)).toBeInTheDocument()
    expect(screen.getByText(agent.config.shell)).toBeInTheDocument()
  })

  it('shows "No" for non-interactive agents', () => {
    const agent = makeAgent({ config: { working_dir: '/tmp', shell: '/bin/bash', interactive: false, tool_policy: { type: 'AllowAll' } } as import('@/types/orchestrator').AgentConfig })
    render(<AgentConfigPanel agent={agent} />)
    expect(screen.getByText('No')).toBeInTheDocument()
  })

  it('shows "Yes (TTY)" for interactive agents', () => {
    const agent = makeAgent({ config: { working_dir: '/tmp', shell: '/bin/bash', interactive: true, tool_policy: { type: 'AllowAll' } } as import('@/types/orchestrator').AgentConfig })
    render(<AgentConfigPanel agent={agent} />)
    expect(screen.getByText('Yes (TTY)')).toBeInTheDocument()
  })

  it('displays AllowAll policy label', () => {
    const agent = makeAgent()
    render(<AgentConfigPanel agent={agent} />)
    expect(screen.getByText('Allow All tools')).toBeInTheDocument()
  })

  it('displays AllowList policy with tools', () => {
    const agent = makeAgent({
      config: {
        working_dir: '/tmp',
        shell: '/bin/bash',
        interactive: false,
        tool_policy: { type: 'AllowList', tools: ['bash', 'read_file'] },
      } as import('@/types/orchestrator').AgentConfig,
    })
    render(<AgentConfigPanel agent={agent} />)
    expect(screen.getByText(/Allow: bash, read_file/)).toBeInTheDocument()
  })

  it('collapses and expands when header is clicked', () => {
    const agent = makeAgent()
    render(<AgentConfigPanel agent={agent} />)

    const toggle = screen.getByRole('button', { name: /configuration/i })

    // Initially open — working_dir visible
    expect(screen.getByText(agent.config.working_dir)).toBeInTheDocument()

    // Collapse
    fireEvent.click(toggle)
    expect(screen.queryByText(agent.config.working_dir)).not.toBeInTheDocument()

    // Re-expand
    fireEvent.click(toggle)
    expect(screen.getByText(agent.config.working_dir)).toBeInTheDocument()
  })

  it('shows system prompt with expand/collapse for long prompts', () => {
    const longPrompt = 'A'.repeat(300)
    const agent = makeAgent({
      config: {
        working_dir: '/tmp',
        shell: '/bin/bash',
        interactive: false,
        tool_policy: { type: 'AllowAll' },
        system_prompt: longPrompt,
      } as import('@/types/orchestrator').AgentConfig,
    })
    render(<AgentConfigPanel agent={agent} />)
    expect(screen.getByText(/show more/i)).toBeInTheDocument()
    fireEvent.click(screen.getByText(/show more/i))
    expect(screen.getByText(/show less/i)).toBeInTheDocument()
  })

  it('masks env variable values by default', () => {
    const agent = makeAgent({
      config: {
        working_dir: '/tmp',
        shell: '/bin/bash',
        interactive: false,
        tool_policy: { type: 'AllowAll' },
        env: { SECRET: 'my-secret-value' },
      } as import('@/types/orchestrator').AgentConfig,
    })
    render(<AgentConfigPanel agent={agent} />)
    // Key visible, value masked
    expect(screen.getByText('SECRET=')).toBeInTheDocument()
    expect(screen.queryByText('my-secret-value')).not.toBeInTheDocument()
  })

  it('reveals env variable values when show button clicked', () => {
    const agent = makeAgent({
      config: {
        working_dir: '/tmp',
        shell: '/bin/bash',
        interactive: false,
        tool_policy: { type: 'AllowAll' },
        env: { SECRET: 'my-secret-value' },
      } as import('@/types/orchestrator').AgentConfig,
    })
    render(<AgentConfigPanel agent={agent} />)
    fireEvent.click(screen.getByRole('button', { name: /show env values/i }))
    expect(screen.getByText('my-secret-value')).toBeInTheDocument()
  })
})
