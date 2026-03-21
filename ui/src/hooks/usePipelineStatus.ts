/**
 * usePipelineStatus — fetches the current autonomous pipeline state.
 *
 * Status: stub / not yet wired.
 *
 * The Conductor agent (v0.10.0, issue #603) will expose pipeline state by
 * writing structured memories tagged conductor+pipeline after each run.
 * This hook will be updated to query the memory service (or a dedicated
 * orchestrator endpoint) once that work lands.
 *
 * Until then it returns { status: null, loading: false } so the
 * PipelineStatusCard renders its "not yet active" empty state instead
 * of a perpetual spinner.
 */

import { useEffect, useState } from 'react'
import type { PipelineStatus } from '@/types/pipeline'

export interface UsePipelineStatusResult {
  status: PipelineStatus | null
  loading: boolean
  error?: string
  /** Manually re-fetch */
  refetch: () => void
}

export function usePipelineStatus(): UsePipelineStatusResult {
  const [status] = useState<PipelineStatus | null>(null)
  const [loading] = useState(false)
  const [error] = useState<string | undefined>(undefined)
  const [, setTick] = useState(0)

  // Placeholder effect — replace with real fetch when conductor endpoint exists.
  useEffect(() => {
    // TODO(v0.10.0): query memory service for latest conductor pipeline digest.
    // Example query:
    //   memoryClient.search('pipeline state merge queue', {
    //     tags: ['conductor', 'pipeline'],
    //     limit: 1,
    //   })
  }, [])

  return {
    status,
    loading,
    error,
    refetch: () => setTick((t) => t + 1),
  }
}
