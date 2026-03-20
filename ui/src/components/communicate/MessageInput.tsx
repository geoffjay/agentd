/**
 * MessageInput — text input area at the bottom of the chat view.
 *
 * Features:
 * - Auto-growing textarea (up to 6 lines)
 * - Enter to send, Shift+Enter for newline
 * - Send button (disabled when empty or sending)
 * - Shows a "Join Room" prompt when the human is not a participant
 */

import { useRef, useState, useCallback } from 'react'
import { Send } from 'lucide-react'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface MessageInputProps {
  /** Called with the trimmed message text when the user sends. */
  onSend: (content: string) => Promise<void>
  /** Whether the human is a participant in the current room. */
  isParticipant: boolean
  /** Called when the user clicks "Join Room". */
  onJoin: () => void
  /** Disables the input (e.g. while joining). */
  joiningRoom?: boolean
  /** Disables the whole input area (no room selected, etc.). */
  disabled?: boolean
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function MessageInput({
  onSend,
  isParticipant,
  onJoin,
  joiningRoom = false,
  disabled = false,
}: MessageInputProps) {
  const [text, setText] = useState('')
  const [sending, setSending] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  const handleSend = useCallback(async () => {
    const trimmed = text.trim()
    if (!trimmed || sending || disabled) return
    setSending(true)
    try {
      await onSend(trimmed)
      setText('')
      // Reset textarea height
      if (textareaRef.current) {
        textareaRef.current.style.height = 'auto'
      }
    } finally {
      setSending(false)
      textareaRef.current?.focus()
    }
  }, [text, sending, disabled, onSend])

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        void handleSend()
      }
    },
    [handleSend],
  )

  // Auto-grow the textarea
  const handleInput = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setText(e.target.value)
    const el = e.target
    el.style.height = 'auto'
    el.style.height = Math.min(el.scrollHeight, 144) + 'px' // max ~6 lines
  }, [])

  // Non-participant: show join prompt
  if (!isParticipant) {
    return (
      <div className="flex shrink-0 items-center justify-between gap-3 border-t border-gray-700 bg-gray-800 px-4 py-3">
        <p className="text-sm text-gray-400">You are not a participant in this room.</p>
        <button
          type="button"
          onClick={onJoin}
          disabled={joiningRoom}
          className="rounded-md bg-emerald-600 px-4 py-1.5 text-sm font-medium text-white hover:bg-emerald-700 transition-colors disabled:opacity-50"
        >
          {joiningRoom ? 'Joining…' : 'Join Room'}
        </button>
      </div>
    )
  }

  return (
    <div className="shrink-0 border-t border-gray-700 bg-gray-800 px-4 py-3">
      <div className="flex items-end gap-2 rounded-lg border border-gray-600 bg-gray-900 px-3 py-2 focus-within:ring-2 focus-within:ring-primary-500 focus-within:border-transparent">
        <textarea
          ref={textareaRef}
          value={text}
          onChange={handleInput}
          onKeyDown={handleKeyDown}
          placeholder="Message… (Enter to send, Shift+Enter for newline)"
          rows={1}
          disabled={sending || disabled}
          aria-label="Message input"
          className="flex-1 resize-none bg-transparent text-sm text-white placeholder-gray-500 focus:outline-none disabled:opacity-50"
          style={{ maxHeight: '144px' }}
        />
        <button
          type="button"
          onClick={() => void handleSend()}
          disabled={sending || disabled || !text.trim()}
          aria-label="Send message"
          className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-primary-600 text-white hover:bg-primary-700 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        >
          <Send size={14} />
        </button>
      </div>
      <p className="mt-1 text-right text-[10px] text-gray-600">
        Enter to send · Shift+Enter for newline
      </p>
    </div>
  )
}
