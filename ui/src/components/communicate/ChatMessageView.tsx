/**
 * ChatMessageView — displays message history for a room.
 *
 * Features:
 * - Loads and renders message history
 * - Auto-scrolls to the latest message on new arrivals
 * - Infinite scroll upward (load older messages)
 * - Visual distinction between agent and human messages
 * - Thread reply indicator for messages with reply_to
 * - Loading states for initial load and older-page load
 */

import { useEffect, useRef, useCallback } from 'react'
import { Bot, User, CornerUpLeft, Loader2 } from 'lucide-react'
import type { ChatMessage, ParticipantKind } from '@/types/communicate'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatTime(iso: string): string {
  return new Intl.DateTimeFormat(undefined, {
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(iso))
}

// ---------------------------------------------------------------------------
// Sender avatar
// ---------------------------------------------------------------------------

function SenderAvatar({ kind }: { kind: ParticipantKind }) {
  return (
    <div
      className={[
        'flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-white',
        kind === 'agent' ? 'bg-primary-600' : 'bg-emerald-600',
      ].join(' ')}
      aria-hidden="true"
    >
      {kind === 'agent' ? <Bot size={16} /> : <User size={16} />}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Kind badge
// ---------------------------------------------------------------------------

function KindBadge({ kind }: { kind: ParticipantKind }) {
  return (
    <span
      className={[
        'rounded px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide',
        kind === 'agent'
          ? 'bg-primary-900 text-primary-300'
          : 'bg-emerald-900 text-emerald-300',
      ].join(' ')}
    >
      {kind}
    </span>
  )
}

// ---------------------------------------------------------------------------
// Single message bubble
// ---------------------------------------------------------------------------

interface MessageBubbleProps {
  message: ChatMessage
  replyToMessage?: ChatMessage
}

function MessageBubble({ message, replyToMessage }: MessageBubbleProps) {
  return (
    <div className="flex items-start gap-3 group">
      <SenderAvatar kind={message.sender_kind} />

      <div className="min-w-0 flex-1">
        {/* Header */}
        <div className="flex items-center gap-2 mb-1">
          <span className="text-sm font-semibold text-white">{message.sender_name}</span>
          <KindBadge kind={message.sender_kind} />
          <span className="text-xs text-gray-500">{formatTime(message.created_at)}</span>
        </div>

        {/* Reply indicator */}
        {replyToMessage && (
          <div className="mb-1 flex items-center gap-1.5 rounded-md border-l-2 border-gray-500 bg-gray-700/50 px-2 py-1 text-xs text-gray-400">
            <CornerUpLeft size={12} className="shrink-0" />
            <span className="font-medium">{replyToMessage.sender_name}</span>
            <span className="truncate">{replyToMessage.content}</span>
          </div>
        )}

        {/* Content */}
        <p className="whitespace-pre-wrap break-words text-sm text-gray-200 leading-relaxed">
          {message.content}
        </p>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// ChatMessageView
// ---------------------------------------------------------------------------

interface ChatMessageViewProps {
  messages: ChatMessage[]
  loading: boolean
  loadingOlder: boolean
  hasMore: boolean
  onLoadOlder: () => void
}

export function ChatMessageView({
  messages,
  loading,
  loadingOlder,
  hasMore,
  onLoadOlder,
}: ChatMessageViewProps) {
  const bottomRef = useRef<HTMLDivElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const prevScrollHeightRef = useRef<number>(0)

  // Auto-scroll to bottom when new messages arrive
  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    // If the user is near the bottom, keep them there
    const distanceFromBottom =
      container.scrollHeight - container.scrollTop - container.clientHeight

    if (distanceFromBottom < 120) {
      bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
    }
  }, [messages])

  // Maintain scroll position when older messages are prepended
  useEffect(() => {
    const container = containerRef.current
    if (!container || !loadingOlder) return
    prevScrollHeightRef.current = container.scrollHeight
  }, [loadingOlder])

  useEffect(() => {
    const container = containerRef.current
    if (!container || loadingOlder || prevScrollHeightRef.current === 0) return
    const delta = container.scrollHeight - prevScrollHeightRef.current
    if (delta > 0) {
      container.scrollTop += delta
    }
    prevScrollHeightRef.current = 0
  }, [messages, loadingOlder])

  // Infinite scroll: fire when user scrolls near the top
  const handleScroll = useCallback(() => {
    const container = containerRef.current
    if (!container || loadingOlder || !hasMore) return
    if (container.scrollTop < 80) {
      onLoadOlder()
    }
  }, [loadingOlder, hasMore, onLoadOlder])

  // Build lookup map for reply references
  const messageMap = new Map(messages.map((m) => [m.id, m]))

  if (loading) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <Loader2 size={24} className="animate-spin text-gray-400" />
      </div>
    )
  }

  if (messages.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <p className="text-sm text-gray-500">No messages yet. Start the conversation!</p>
      </div>
    )
  }

  return (
    <div
      ref={containerRef}
      onScroll={handleScroll}
      className="flex-1 overflow-y-auto px-4 py-4 space-y-4"
      aria-label="Chat messages"
      aria-live="polite"
      aria-relevant="additions"
    >
      {/* Load older indicator */}
      {loadingOlder && (
        <div className="flex justify-center py-2">
          <Loader2 size={16} className="animate-spin text-gray-400" />
        </div>
      )}
      {!hasMore && messages.length > 0 && (
        <p className="text-center text-xs text-gray-600 py-1">Beginning of conversation</p>
      )}

      {messages.map((msg) => (
        <MessageBubble
          key={msg.id}
          message={msg}
          replyToMessage={msg.reply_to ? messageMap.get(msg.reply_to) : undefined}
        />
      ))}

      <div ref={bottomRef} />
    </div>
  )
}
