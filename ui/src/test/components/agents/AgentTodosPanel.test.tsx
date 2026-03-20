import { describe, it, expect } from 'vitest'
import { render, screen, act } from '@testing-library/react'
import { AgentTodosPanel } from '@/components/agents/AgentTodosPanel'
import { agentEventBus } from '@/services/eventBus'
import type { AgentToolUseEvent } from '@/types/orchestrator'

const AGENT_ID = 'test-agent-123'
const OTHER_AGENT_ID = 'other-agent-456'

function makeTodoWriteEvent(
  agentId: string,
  todos: Array<{ id: string; content: string; status: string; priority: string }>,
): AgentToolUseEvent {
  return {
    type: 'agent:tool_use',
    agentId,
    tool_name: 'TodoWrite',
    tool_id: 'tw-1',
    tool_input: { todos },
    summary: 'Update todos',
    timestamp: new Date().toISOString(),
  }
}

describe('AgentTodosPanel', () => {
  it('renders nothing before any TodoWrite event is received', () => {
    const { container } = render(<AgentTodosPanel agentId={AGENT_ID} />)
    expect(container.firstChild).toBeNull()
  })

  it('renders the panel after a TodoWrite event arrives', () => {
    render(<AgentTodosPanel agentId={AGENT_ID} />)
    act(() => {
      agentEventBus.emit(
        makeTodoWriteEvent(AGENT_ID, [
          { id: '1', content: 'Write tests', status: 'pending', priority: 'medium' },
        ]),
      )
    })
    expect(screen.getByRole('region', { name: /agent todos/i })).toBeInTheDocument()
    expect(screen.getByText('Write tests')).toBeInTheDocument()
  })

  it('ignores events from other agents', () => {
    const { container } = render(<AgentTodosPanel agentId={AGENT_ID} />)
    act(() => {
      agentEventBus.emit(
        makeTodoWriteEvent(OTHER_AGENT_ID, [
          { id: '1', content: 'Should not appear', status: 'pending', priority: 'medium' },
        ]),
      )
    })
    expect(container.firstChild).toBeNull()
  })

  it('ignores non-TodoWrite tool_use events', () => {
    const { container } = render(<AgentTodosPanel agentId={AGENT_ID} />)
    act(() => {
      agentEventBus.emit({
        type: 'agent:tool_use',
        agentId: AGENT_ID,
        tool_name: 'Bash',
        tool_id: 'bash-1',
        tool_input: { command: 'echo hello' },
        summary: 'echo hello',
        timestamp: new Date().toISOString(),
      } satisfies AgentToolUseEvent)
    })
    expect(container.firstChild).toBeNull()
  })

  it('updates todos when a new TodoWrite event arrives', () => {
    render(<AgentTodosPanel agentId={AGENT_ID} />)
    act(() => {
      agentEventBus.emit(
        makeTodoWriteEvent(AGENT_ID, [
          { id: '1', content: 'First task', status: 'pending', priority: 'medium' },
        ]),
      )
    })
    expect(screen.getByText('First task')).toBeInTheDocument()

    act(() => {
      agentEventBus.emit(
        makeTodoWriteEvent(AGENT_ID, [
          { id: '1', content: 'Updated task', status: 'in_progress', priority: 'high' },
        ]),
      )
    })
    expect(screen.queryByText('First task')).not.toBeInTheDocument()
    expect(screen.getByText('Updated task')).toBeInTheDocument()
  })

  it('shows completion count in the header', () => {
    render(<AgentTodosPanel agentId={AGENT_ID} />)
    act(() => {
      agentEventBus.emit(
        makeTodoWriteEvent(AGENT_ID, [
          { id: '1', content: 'Done', status: 'completed', priority: 'low' },
          { id: '2', content: 'Pending', status: 'pending', priority: 'medium' },
          { id: '3', content: 'Active', status: 'in_progress', priority: 'high' },
        ]),
      )
    })
    // header badge shows "completed/total"
    expect(screen.getByText('1/3')).toBeInTheDocument()
    // active badge shows in-progress count
    expect(screen.getByText('1 active')).toBeInTheDocument()
  })

  it('applies strikethrough style for completed todos', () => {
    render(<AgentTodosPanel agentId={AGENT_ID} />)
    act(() => {
      agentEventBus.emit(
        makeTodoWriteEvent(AGENT_ID, [
          { id: '1', content: 'Done task', status: 'completed', priority: 'medium' },
        ]),
      )
    })
    const text = screen.getByText('Done task')
    expect(text.className).toMatch(/line-through/)
  })

  it('shows priority badge for high priority items', () => {
    render(<AgentTodosPanel agentId={AGENT_ID} />)
    act(() => {
      agentEventBus.emit(
        makeTodoWriteEvent(AGENT_ID, [
          { id: '1', content: 'Urgent task', status: 'pending', priority: 'high' },
        ]),
      )
    })
    expect(screen.getByText('high')).toBeInTheDocument()
  })

  it('shows priority badge for low priority items', () => {
    render(<AgentTodosPanel agentId={AGENT_ID} />)
    act(() => {
      agentEventBus.emit(
        makeTodoWriteEvent(AGENT_ID, [
          { id: '1', content: 'Low priority task', status: 'pending', priority: 'low' },
        ]),
      )
    })
    expect(screen.getByText('low')).toBeInTheDocument()
  })

  it('does not show a priority badge for medium priority items', () => {
    render(<AgentTodosPanel agentId={AGENT_ID} />)
    act(() => {
      agentEventBus.emit(
        makeTodoWriteEvent(AGENT_ID, [
          { id: '1', content: 'Normal task', status: 'pending', priority: 'medium' },
        ]),
      )
    })
    expect(screen.queryByText('medium')).not.toBeInTheDocument()
  })

  it('shows "No todos." when the todo list is empty', () => {
    render(<AgentTodosPanel agentId={AGENT_ID} />)
    act(() => {
      agentEventBus.emit(makeTodoWriteEvent(AGENT_ID, []))
    })
    expect(screen.getByText('No todos.')).toBeInTheDocument()
  })

  it('does not show the active badge when no todos are in progress', () => {
    render(<AgentTodosPanel agentId={AGENT_ID} />)
    act(() => {
      agentEventBus.emit(
        makeTodoWriteEvent(AGENT_ID, [
          { id: '1', content: 'Pending task', status: 'pending', priority: 'medium' },
        ]),
      )
    })
    expect(screen.queryByText(/active/)).not.toBeInTheDocument()
  })
})
