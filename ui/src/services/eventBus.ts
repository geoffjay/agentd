/**
 * agentEventBus — central pub/sub bus for WebSocket-sourced agent events.
 *
 * Components and hooks can subscribe to specific event types without
 * knowing which WebSocket connection produced them.
 *
 * Events are published by useAllAgentsStream as it parses messages
 * from the /stream endpoint.
 */

import type { AgentEvent } from '@/types/orchestrator'

type AnyHandler = (event: AgentEvent) => void

class EventBus {
  private readonly listeners = new Map<string, Set<AnyHandler>>()

  /**
   * Publish an event to all registered handlers for its type.
   */
  emit<T extends AgentEvent>(event: T): void {
    const handlers = this.listeners.get(event.type)
    if (!handlers) return
    for (const handler of handlers) {
      handler(event)
    }
  }

  /**
   * Subscribe to events of a specific type.
   * @returns Cleanup function that removes the handler.
   */
  on<T extends AgentEvent>(type: T['type'], handler: (event: T) => void): () => void {
    let handlers = this.listeners.get(type)
    if (!handlers) {
      handlers = new Set()
      this.listeners.set(type, handlers)
    }
    handlers.add(handler as AnyHandler)
    return () => {
      handlers!.delete(handler as AnyHandler)
    }
  }
}

/** Singleton event bus — import and use this directly */
export const agentEventBus = new EventBus()
