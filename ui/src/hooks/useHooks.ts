/**
 * useHooks — stub hook returning empty data for the hooks management page.
 *
 * This is a placeholder for when the hook service (port 17002) is fully
 * implemented. The hook service will monitor git hooks and system hooks
 * and create notifications based on hook events.
 *
 * Replace the stub implementations below with real API calls once the
 * hook service is available.
 */

import { useState } from 'react'
import type { Hook, HookEvent } from '@/types/hook'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface UseHooksResult {
  hooks: Hook[]
  total: number
  loading: boolean
  error?: string
  refetch: () => void
}

export interface UseHookEventsResult {
  events: HookEvent[]
  total: number
  loading: boolean
  error?: string
  refetch: () => void
}

export interface UseHookServiceStatusResult {
  /** Whether the hook service is reachable */
  reachable: boolean
  checking: boolean
  /** Port the hook service listens on */
  port: number
}

// ---------------------------------------------------------------------------
// Hook service port constant
// ---------------------------------------------------------------------------

/** The hook service listens on port 17002 (not yet implemented) */
export const HOOK_SERVICE_PORT = 17002

// ---------------------------------------------------------------------------
// useHooks — list all registered hooks
// ---------------------------------------------------------------------------

/**
 * Returns the list of registered hooks from the hook service.
 *
 * Currently returns empty data since the service is not yet implemented.
 * Replace with a real API call when the hook service is available.
 */
export function useHooks(): UseHooksResult {
  const [loading] = useState(false)

  return {
    hooks: [],
    total: 0,
    loading,
    error: undefined,
    refetch: () => {
      // TODO: fetch from hook service when implemented
    },
  }
}

// ---------------------------------------------------------------------------
// useHookEvents — list recent hook execution events
// ---------------------------------------------------------------------------

/**
 * Returns recent hook execution events from the hook service.
 *
 * Currently returns empty data since the service is not yet implemented.
 */
export function useHookEvents(): UseHookEventsResult {
  const [loading] = useState(false)

  return {
    events: [],
    total: 0,
    loading,
    error: undefined,
    refetch: () => {
      // TODO: fetch from hook service when implemented
    },
  }
}

// ---------------------------------------------------------------------------
// useHookServiceStatus — check if the hook service is reachable
// ---------------------------------------------------------------------------

/**
 * Checks whether the hook service is reachable.
 *
 * Always returns unreachable since the service is not yet implemented.
 * Replace with a real health-check call when the hook service is available.
 */
export function useHookServiceStatus(): UseHookServiceStatusResult {
  return {
    reachable: false,
    checking: false,
    port: HOOK_SERVICE_PORT,
  }
}
