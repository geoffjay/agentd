/**
 * WebSocketManager — full lifecycle management for WebSocket connections.
 *
 * Features:
 * - Connection states: Connecting | Connected | Disconnected | Reconnecting
 * - Auto-reconnect with exponential backoff (min 1 s, max 30 s)
 * - Heartbeat/ping to detect stale connections
 * - Message buffering during reconnection (up to bufferSize messages)
 * - Typed message and state change handler registration
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type ConnectionState = 'Connecting' | 'Connected' | 'Disconnected' | 'Reconnecting'

type MessageHandler = (event: MessageEvent) => void
type StateHandler = (state: ConnectionState) => void

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DEFAULT_MIN_DELAY = 1_000
const DEFAULT_MAX_DELAY = 30_000
const DEFAULT_HEARTBEAT_INTERVAL = 15_000
const DEFAULT_BUFFER_SIZE = 256

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface WebSocketManagerOptions {
  /** Minimum reconnect delay in ms (default 1 000) */
  minReconnectDelay?: number
  /** Maximum reconnect delay in ms (default 30 000) */
  maxReconnectDelay?: number
  /**
   * Heartbeat interval in ms. Set to 0 to disable.
   * When enabled a "ping" text frame is sent at the given interval to keep
   * the connection alive and detect stale sockets. (default 15 000)
   */
  heartbeatInterval?: number
  /** Max messages to buffer while reconnecting (default 256) */
  messageBufferSize?: number
}

// ---------------------------------------------------------------------------
// WebSocketManager
// ---------------------------------------------------------------------------

export class WebSocketManager {
  private ws: WebSocket | null = null
  private _state: ConnectionState = 'Disconnected'
  private readonly messageHandlers = new Set<MessageHandler>()
  private readonly stateHandlers = new Set<StateHandler>()
  /** Messages buffered while the socket is not open */
  private readonly pendingBuffer: string[] = []

  private reconnectTimer: ReturnType<typeof setTimeout> | null = null
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null
  private reconnectDelay: number
  private intentionalClose = false

  private readonly minDelay: number
  private readonly maxDelay: number
  private readonly heartbeatInterval: number
  private readonly bufferSize: number

  constructor(
    private readonly url: string,
    options: WebSocketManagerOptions = {},
  ) {
    this.minDelay = options.minReconnectDelay ?? DEFAULT_MIN_DELAY
    this.maxDelay = options.maxReconnectDelay ?? DEFAULT_MAX_DELAY
    this.heartbeatInterval = options.heartbeatInterval ?? DEFAULT_HEARTBEAT_INTERVAL
    this.bufferSize = options.messageBufferSize ?? DEFAULT_BUFFER_SIZE
    this.reconnectDelay = this.minDelay
  }

  // -------------------------------------------------------------------------
  // Public API
  // -------------------------------------------------------------------------

  /** Current connection state */
  get state(): ConnectionState {
    return this._state
  }

  /**
   * Start the connection. Idempotent — calling while already Connecting or
   * Connected is a no-op.
   */
  connect(): void {
    if (
      this.ws &&
      (this.ws.readyState === WebSocket.CONNECTING ||
        this.ws.readyState === WebSocket.OPEN)
    ) {
      return
    }
    this.intentionalClose = false
    this._open()
  }

  /** Close the connection and stop any pending reconnect attempts. */
  disconnect(): void {
    this.intentionalClose = true
    this._clearTimers()
    if (this.ws) {
      this.ws.onclose = null
      this.ws.onerror = null
      this.ws.onmessage = null
      this.ws.onopen = null
      this.ws.close()
      this.ws = null
    }
    this._setState('Disconnected')
  }

  /**
   * Send a message. If the socket is not currently open the message is
   * buffered (up to messageBufferSize) and flushed once reconnected.
   */
  send(message: string): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(message)
    } else {
      if (this.pendingBuffer.length < this.bufferSize) {
        this.pendingBuffer.push(message)
      }
    }
  }

  /**
   * Register a message handler.
   * @returns Cleanup function that removes the handler.
   */
  onMessage(handler: MessageHandler): () => void {
    this.messageHandlers.add(handler)
    return () => { this.messageHandlers.delete(handler) }
  }

  /**
   * Register a connection-state change handler.
   * @returns Cleanup function that removes the handler.
   */
  onStateChange(handler: StateHandler): () => void {
    this.stateHandlers.add(handler)
    return () => { this.stateHandlers.delete(handler) }
  }

  // -------------------------------------------------------------------------
  // Private helpers
  // -------------------------------------------------------------------------

  private _open(): void {
    this._setState(
      this._state === 'Disconnected' ? 'Connecting' : 'Reconnecting',
    )

    try {
      this.ws = new WebSocket(this.url)
    } catch {
      this._scheduleReconnect()
      return
    }

    this.ws.onopen = () => {
      this.reconnectDelay = this.minDelay
      this._setState('Connected')
      this._flushBuffer()
      this._startHeartbeat()
    }

    this.ws.onmessage = (event: MessageEvent) => {
      for (const handler of this.messageHandlers) {
        handler(event)
      }
    }

    // onerror is always followed by onclose; let onclose handle reconnect
    this.ws.onerror = () => {}

    this.ws.onclose = () => {
      this._clearHeartbeat()
      if (this.intentionalClose) {
        this._setState('Disconnected')
      } else {
        this._setState('Reconnecting')
        this._scheduleReconnect()
      }
    }
  }

  private _setState(state: ConnectionState): void {
    if (this._state === state) return
    this._state = state
    for (const handler of this.stateHandlers) {
      handler(state)
    }
  }

  private _scheduleReconnect(): void {
    this._clearReconnectTimer()
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null
      if (!this.intentionalClose) {
        this._open()
      }
    }, this.reconnectDelay)
    this.reconnectDelay = Math.min(this.reconnectDelay * 2, this.maxDelay)
  }

  private _flushBuffer(): void {
    const pending = this.pendingBuffer.splice(0)
    for (const msg of pending) {
      this.ws?.send(msg)
    }
  }

  private _startHeartbeat(): void {
    if (this.heartbeatInterval <= 0) return
    this._clearHeartbeat()
    this.heartbeatTimer = setInterval(() => {
      if (this.ws?.readyState === WebSocket.OPEN) {
        this.ws.send('ping')
      }
    }, this.heartbeatInterval)
  }

  private _clearHeartbeat(): void {
    if (this.heartbeatTimer !== null) {
      clearInterval(this.heartbeatTimer)
      this.heartbeatTimer = null
    }
  }

  private _clearReconnectTimer(): void {
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer)
      this.reconnectTimer = null
    }
  }

  private _clearTimers(): void {
    this._clearReconnectTimer()
    this._clearHeartbeat()
  }
}
