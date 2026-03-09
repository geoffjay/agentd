/**
 * useWebSocket — low-level React hook wrapping WebSocketManager.
 *
 * Auto-connects on mount and disconnects on unmount.
 * Re-connects whenever the URL changes.
 *
 * Returns:
 *   messages        - received MessageEvents (newest-last, capped at maxMessages)
 *   connectionState - current ConnectionState
 *   send            - send a text message (buffered while disconnected)
 *   disconnect      - manually close the connection
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { WebSocketManager } from '@/services/websocket'
import type { ConnectionState, WebSocketManagerOptions } from '@/services/websocket'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type { ConnectionState }

export interface UseWebSocketResult {
  /** Received MessageEvents — newest last, capped at maxMessages */
  messages: MessageEvent[]
  connectionState: ConnectionState
  /** Send a text message; buffers up to messageBufferSize msgs if disconnected */
  send: (message: string) => void
  /** Manually close the connection */
  disconnect: () => void
}

export interface UseWebSocketOptions extends WebSocketManagerOptions {
  /** Max number of messages to retain in state (default 200) */
  maxMessages?: number
  /** When true, suppresses auto-connect (default false) */
  paused?: boolean
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useWebSocket(url: string, options: UseWebSocketOptions = {}): UseWebSocketResult {
  const { maxMessages = 200, paused = false, ...managerOpts } = options

  const managerRef = useRef<WebSocketManager | null>(null)
  const maxRef = useRef(maxMessages)
  maxRef.current = maxMessages

  const [messages, setMessages] = useState<MessageEvent[]>([])
  const [connectionState, setConnectionState] = useState<ConnectionState>('Disconnected')

  const send = useCallback((message: string) => {
    managerRef.current?.send(message)
  }, [])

  const disconnect = useCallback(() => {
    managerRef.current?.disconnect()
  }, [])

  useEffect(() => {
    if (paused) return

    const manager = new WebSocketManager(url, managerOpts)
    managerRef.current = manager

    const unsubState = manager.onStateChange(setConnectionState)
    const unsubMsg = manager.onMessage((event: MessageEvent) => {
      setMessages((prev) => {
        const next = [...prev, event]
        const max = maxRef.current
        return next.length > max ? next.slice(next.length - max) : next
      })
    })

    manager.connect()

    return () => {
      unsubState()
      unsubMsg()
      manager.disconnect()
      managerRef.current = null
    }
    // managerOpts is intentionally omitted — URL/paused changes drive reconnect
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [url, paused])

  return { messages, connectionState, send, disconnect }
}
