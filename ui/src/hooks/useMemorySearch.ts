/**
 * useMemorySearch — hook for semantic memory search.
 *
 * Provides:
 * - Execute semantic similarity search against the memory service
 * - Clear results to reset the search state
 * - Loading and error state management
 * - Error handling with mapApiError and toast notifications
 */

import { useCallback, useState } from 'react'
import { memoryClient } from '@/services/memory'
import { useToast, mapApiError } from '@/hooks/useToast'
import type { Memory, SearchRequest } from '@/types/memory'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface UseMemorySearchResult {
  /** Matching memory records, ordered by similarity score. */
  results: Memory[]
  /** Total number of matches from the last search. */
  total: number
  /** True while a search request is in flight. */
  searching: boolean
  /** Error message from the last search, if any. */
  error?: string
  /** Execute a semantic similarity search. */
  search: (request: SearchRequest) => Promise<void>
  /** Clear results and reset to the initial state. */
  clear: () => void
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useMemorySearch(): UseMemorySearchResult {
  const [results, setResults] = useState<Memory[]>([])
  const [total, setTotal] = useState(0)
  const [searching, setSearching] = useState(false)
  const [error, setError] = useState<string | undefined>()
  const toast = useToast()

  const search = useCallback(
    async (request: SearchRequest): Promise<void> => {
      setSearching(true)
      setError(undefined)
      try {
        const response = await memoryClient.searchMemories(request)
        setResults(response.memories)
        setTotal(response.total)
      } catch (err) {
        const msg = mapApiError(err)
        setError(msg)
        toast.apiError(err, 'Memory search failed')
      } finally {
        setSearching(false)
      }
    },
    [toast],
  )

  const clear = useCallback(() => {
    setResults([])
    setTotal(0)
    setError(undefined)
  }, [])

  return {
    results,
    total,
    searching,
    error,
    search,
    clear,
  }
}
