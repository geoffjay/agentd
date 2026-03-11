/**
 * AgentCommandInput — message input for non-interactive running agents.
 *
 * Features:
 * - Only enabled when agent is Running + non-interactive
 * - Send on Enter key or Send button click
 * - Command history: up/down arrow navigation (stored in sessionStorage)
 * - Loading state while message is being sent
 * - Error display inline
 */

import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { Send } from 'lucide-react'

// ---------------------------------------------------------------------------
// Command history (sessionStorage-backed)
// ---------------------------------------------------------------------------

const HISTORY_KEY = (agentId: string) => `agentd:cmd-history:${agentId}`
const MAX_HISTORY = 100

function loadHistory(agentId: string): string[] {
  try {
    const raw = sessionStorage.getItem(HISTORY_KEY(agentId))
    return raw ? (JSON.parse(raw) as string[]) : []
  } catch {
    return []
  }
}

function saveHistory(agentId: string, history: string[]): void {
  try {
    sessionStorage.setItem(HISTORY_KEY(agentId), JSON.stringify(history.slice(-MAX_HISTORY)))
  } catch {
    // ignore quota errors
  }
}

// ---------------------------------------------------------------------------
// AgentCommandInput
// ---------------------------------------------------------------------------

export interface AgentCommandInputProps {
  agentId: string
  /** Whether the command input should be usable */
  enabled: boolean
  /** Reason for disabled state — shown as tooltip/hint */
  disabledReason?: string
  onSend: (message: string) => Promise<void>
}

export function AgentCommandInput({
  agentId,
  enabled,
  disabledReason,
  onSend,
}: AgentCommandInputProps) {
  const [value, setValue] = useState('')
  const [sending, setSending] = useState(false)
  const [error, setError] = useState<string | undefined>()
  const [successFlash, setSuccessFlash] = useState(false)

  const historyRef = useRef<string[]>(loadHistory(agentId))
  /** -1 = not browsing, otherwise index into history from the end */
  const historyIndexRef = useRef(-1)
  const inputRef = useRef<HTMLTextAreaElement>(null)

  const MIN_ROWS = 3
  const MAX_ROWS = 10

  // Reload history when agentId changes
  useEffect(() => {
    historyRef.current = loadHistory(agentId)
    historyIndexRef.current = -1
  }, [agentId])

  // Auto-resize textarea between MIN_ROWS and MAX_ROWS
  const autoResize = useCallback(() => {
    const el = inputRef.current
    if (!el) return
    const lineHeight = parseInt(getComputedStyle(el).lineHeight || '16', 10)
    el.style.height = 'auto'
    const maxHeight = lineHeight * MAX_ROWS
    const minHeight = lineHeight * MIN_ROWS
    el.style.height = `${Math.min(Math.max(el.scrollHeight, minHeight), maxHeight)}px`
  }, [])

  useLayoutEffect(() => {
    autoResize()
  }, [value, autoResize])

  const handleSend = useCallback(async () => {
    const trimmed = value.trim()
    if (!trimmed || sending || !enabled) return

    setSending(true)
    setError(undefined)

    try {
      await onSend(trimmed)

      // Push to history (avoid consecutive duplicates)
      const history = historyRef.current
      if (history[history.length - 1] !== trimmed) {
        history.push(trimmed)
        saveHistory(agentId, history)
      }
      historyIndexRef.current = -1

      setValue('')
      setSuccessFlash(true)
      setTimeout(() => setSuccessFlash(false), 600)
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to send message'
      setError(msg)
    } finally {
      setSending(false)
      inputRef.current?.focus()
    }
  }, [value, sending, enabled, onSend, agentId])

  function handleKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSend()
      return
    }

    const history = historyRef.current
    if (history.length === 0) return

    const el = inputRef.current
    const cursorAtStart = el ? el.selectionStart === 0 && el.selectionEnd === 0 : true
    const cursorAtEnd = el ? el.selectionStart === el.value.length : true

    if (e.key === 'ArrowUp' && cursorAtStart) {
      e.preventDefault()
      const nextIdx =
        historyIndexRef.current === -1
          ? history.length - 1
          : Math.max(0, historyIndexRef.current - 1)
      historyIndexRef.current = nextIdx
      setValue(history[nextIdx] ?? '')
    }

    if (e.key === 'ArrowDown' && cursorAtEnd) {
      e.preventDefault()
      if (historyIndexRef.current === -1) return
      const nextIdx = historyIndexRef.current + 1
      if (nextIdx >= history.length) {
        historyIndexRef.current = -1
        setValue('')
      } else {
        historyIndexRef.current = nextIdx
        setValue(history[nextIdx] ?? '')
      }
    }
  }

  const borderColor = successFlash
    ? 'border-green-500'
    : error
      ? 'border-red-500'
      : 'border-gray-700'

  return (
    <div className="flex flex-col gap-1">
      {error && (
        <p role="alert" className="text-xs text-red-400">
          {error}
        </p>
      )}
      <div
        className={[
          'flex items-start gap-2 rounded-lg border bg-gray-900 px-3 py-2 transition-colors',
          borderColor,
          !enabled ? 'opacity-60' : '',
        ].join(' ')}
        title={!enabled ? disabledReason : undefined}
      >
        <span className="mt-1 shrink-0 select-none font-mono text-xs text-gray-500">$</span>
        <textarea
          ref={inputRef}
          rows={MIN_ROWS}
          aria-label="Send message to agent"
          placeholder={
            !enabled ? (disabledReason ?? 'Unavailable') : 'Type a message and press Enter…'
          }
          value={value}
          disabled={!enabled || sending}
          onChange={(e) => {
            setValue(e.target.value)
            historyIndexRef.current = -1
          }}
          onKeyDown={handleKeyDown}
          className="flex-1 resize-none mt-1 overflow-y-auto bg-transparent font-mono text-xs leading-4 text-gray-200 placeholder:text-gray-600 focus:outline-none disabled:cursor-not-allowed"
        />
        <button
          type="button"
          aria-label="Send message"
          onClick={handleSend}
          disabled={!enabled || sending || !value.trim()}
          className="mt-1 shrink-0 self-end rounded p-1 text-gray-400 hover:bg-gray-700 hover:text-white disabled:cursor-not-allowed disabled:opacity-40"
        >
          <Send size={13} aria-hidden="true" />
        </button>
      </div>
    </div>
  )
}

export default AgentCommandInput
