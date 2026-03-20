/**
 * useHumanIdentity — manages the local human participant identity.
 *
 * Persists { identifier, displayName } to localStorage so the human's
 * identity survives page refreshes. Returns a null identity until the
 * user completes first-time setup.
 */

import { useState, useCallback } from 'react'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface HumanIdentity {
  identifier: string
  displayName: string
}

export interface UseHumanIdentityResult {
  /** Null until setup is completed. */
  identity: HumanIdentity | null
  /** True when identity has been configured. */
  isSetup: boolean
  /** Persist a new identity and update state. */
  setup: (identifier: string, displayName: string) => void
  /** Clear the identity from localStorage and reset state. */
  clear: () => void
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

export const IDENTITY_KEY = 'agentd:communicate:identity'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function readIdentity(): HumanIdentity | null {
  try {
    const raw = localStorage.getItem(IDENTITY_KEY)
    if (!raw) return null
    const parsed = JSON.parse(raw) as { identifier?: string; displayName?: string }
    if (parsed.identifier && parsed.displayName) {
      return { identifier: parsed.identifier, displayName: parsed.displayName }
    }
  } catch {
    // Ignore parse errors
  }
  return null
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useHumanIdentity(): UseHumanIdentityResult {
  const [identity, setIdentity] = useState<HumanIdentity | null>(readIdentity)

  const setup = useCallback((identifier: string, displayName: string) => {
    const id: HumanIdentity = { identifier, displayName }
    localStorage.setItem(IDENTITY_KEY, JSON.stringify(id))
    setIdentity(id)
  }, [])

  const clear = useCallback(() => {
    localStorage.removeItem(IDENTITY_KEY)
    setIdentity(null)
  }, [])

  return { identity, isSetup: identity !== null, setup, clear }
}
