/**
 * Tests for ChatMessageView component.
 */

import { render, screen } from '@testing-library/react'
import { ChatMessageView } from '@/components/communicate/ChatMessageView'
import { makeChatMessageList, makeChatMessage } from '@/test/mocks/factories'

const noop = () => {}

describe('ChatMessageView', () => {
  it('renders messages', () => {
    const messages = makeChatMessageList(3)
    render(
      <ChatMessageView
        messages={messages}
        loading={false}
        loadingOlder={false}
        hasMore={false}
        onLoadOlder={noop}
      />,
    )

    messages.forEach((msg) => {
      expect(screen.getByText(msg.content)).toBeInTheDocument()
      expect(screen.getByText(msg.sender_name)).toBeInTheDocument()
    })
  })

  it('shows loading spinner when loading', () => {
    render(
      <ChatMessageView
        messages={[]}
        loading={true}
        loadingOlder={false}
        hasMore={false}
        onLoadOlder={noop}
      />,
    )

    // Spinner is present (no messages shown)
    expect(screen.queryByRole('region')).not.toBeInTheDocument()
    // The container with aria-label="Chat messages" should not be present
    expect(screen.queryByLabelText('Chat messages')).not.toBeInTheDocument()
  })

  it('shows empty state when no messages', () => {
    render(
      <ChatMessageView
        messages={[]}
        loading={false}
        loadingOlder={false}
        hasMore={false}
        onLoadOlder={noop}
      />,
    )

    expect(screen.getByText(/no messages yet/i)).toBeInTheDocument()
  })

  it('shows "beginning of conversation" when hasMore is false and messages exist', () => {
    const messages = makeChatMessageList(2)
    render(
      <ChatMessageView
        messages={messages}
        loading={false}
        loadingOlder={false}
        hasMore={false}
        onLoadOlder={noop}
      />,
    )

    expect(screen.getByText(/beginning of conversation/i)).toBeInTheDocument()
  })

  it('shows agent and human kind badges', () => {
    const agentMsg = makeChatMessage({ sender_kind: 'agent', sender_name: 'MyAgent' })
    const humanMsg = makeChatMessage({ sender_kind: 'human', sender_name: 'MyHuman' })

    render(
      <ChatMessageView
        messages={[agentMsg, humanMsg]}
        loading={false}
        loadingOlder={false}
        hasMore={false}
        onLoadOlder={noop}
      />,
    )

    const badges = screen.getAllByText(/agent|human/i)
    // At least one 'agent' badge and one 'human' badge
    expect(badges.some((b) => b.textContent === 'agent')).toBe(true)
    expect(badges.some((b) => b.textContent === 'human')).toBe(true)
  })

  it('shows reply indicator when reply_to is set', () => {
    const parent = makeChatMessage({ content: 'Original message' })
    const reply = makeChatMessage({ reply_to: parent.id, content: 'Reply message' })

    render(
      <ChatMessageView
        messages={[parent, reply]}
        loading={false}
        loadingOlder={false}
        hasMore={false}
        onLoadOlder={noop}
      />,
    )

    // The parent content appears in the reply indicator
    expect(screen.getAllByText('Original message')).toHaveLength(2)
    expect(screen.getByText('Reply message')).toBeInTheDocument()
  })
})
