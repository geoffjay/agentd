/**
 * useSearch — global search hook with debouncing, parallel API calls, and
 * result ranking.
 *
 * Features:
 * - 300 ms debounce before issuing API calls
 * - Parallel searches across Agents and Notifications
 * - Client-side quick-action filtering
 * - Relevance ranking: exact > prefix > contains
 * - Max 5 results per category
 * - Recent-search history persisted to localStorage
 */

import { useState, useCallback, useEffect, useRef } from 'react'
import { orchestratorClient } from '@/services/orchestrator'
import { notifyClient } from '@/services/notify'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type SearchCategory = 'agent' | 'notification' | 'action'

export interface SearchResult {
  id: string
  category: SearchCategory
  title: string
  subtitle: string
  href: string
}

export interface GroupedSearchResults {
  actions: SearchResult[]
  agents: SearchResult[]
  notifications: SearchResult[]
  /** Total across all groups */
  total: number
}

export interface UseSearchResult {
  query: string
  setQuery: (q: string) => void
  results: GroupedSearchResults
  loading: boolean
  recentSearches: string[]
  addRecentSearch: (q: string) => void
  clearRecentSearches: () => void
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const RECENT_SEARCHES_KEY = 'agentd:recent-searches'
const MAX_RECENT = 5
const MAX_PER_CATEGORY = 5
const DEBOUNCE_MS = 300

// ---------------------------------------------------------------------------
// Quick actions (always available, filtered by query)
// ---------------------------------------------------------------------------

const QUICK_ACTIONS: SearchResult[] = [
  {
    id: 'action-dashboard',
    category: 'action',
    title: 'Go to Dashboard',
    subtitle: 'Navigate to the home dashboard',
    href: '/',
  },
  {
    id: 'action-agents',
    category: 'action',
    title: 'Go to Agents',
    subtitle: 'Navigate to the agents page',
    href: '/agents',
  },
  {
    id: 'action-create-agent',
    category: 'action',
    title: 'Create Agent',
    subtitle: 'Open create agent dialog',
    href: '/agents?action=create',
  },
  {
    id: 'action-approvals',
    category: 'action',
    title: 'View Approvals',
    subtitle: 'Navigate to pending approvals',
    href: '/agents?tab=approvals',
  },
  {
    id: 'action-notifications',
    category: 'action',
    title: 'Go to Notifications',
    subtitle: 'Navigate to the notifications page',
    href: '/notifications',
  },
  {
    id: 'action-questions',
    category: 'action',
    title: 'Go to Questions',
    subtitle: 'Navigate to the questions page',
    href: '/questions',
  },
  {
    id: 'action-workflows',
    category: 'action',
    title: 'Go to Workflows',
    subtitle: 'Navigate to the workflows page',
    href: '/workflows',
  },
  {
    id: 'action-monitoring',
    category: 'action',
    title: 'Go to Monitoring',
    subtitle: 'Navigate to the monitoring page',
    href: '/monitoring',
  },
  {
    id: 'action-settings',
    category: 'action',
    title: 'Go to Settings',
    subtitle: 'Open application settings',
    href: '/settings',
  },
  {
    id: 'action-run-checks',
    category: 'action',
    title: 'Run Checks',
    subtitle: 'Trigger the ask service health checks',
    href: '/questions?action=run',
  },
]

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Returns 3 = exact, 2 = prefix, 1 = contains, 0 = no match */
function scoreMatch(text: string, query: string): number {
  const lower = text.toLowerCase()
  const q = query.toLowerCase()
  if (lower === q) return 3
  if (lower.startsWith(q)) return 2
  if (lower.includes(q)) return 1
  return 0
}

function loadRecentSearches(): string[] {
  try {
    const stored = localStorage.getItem(RECENT_SEARCHES_KEY)
    if (!stored) return []
    return JSON.parse(stored) as string[]
  } catch {
    return []
  }
}

function saveRecentSearches(searches: string[]): void {
  try {
    localStorage.setItem(RECENT_SEARCHES_KEY, JSON.stringify(searches))
  } catch {
    // ignore storage errors
  }
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

const EMPTY_RESULTS: GroupedSearchResults = {
  actions: [],
  agents: [],
  notifications: [],
  total: 0,
}

export function useSearch(): UseSearchResult {
  const [query, setQuery] = useState('')
  const [debouncedQuery, setDebouncedQuery] = useState('')
  const [results, setResults] = useState<GroupedSearchResults>(EMPTY_RESULTS)
  const [loading, setLoading] = useState(false)
  const [recentSearches, setRecentSearches] = useState<string[]>(loadRecentSearches)
  const abortRef = useRef<AbortController | null>(null)

  // Debounce the raw query
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedQuery(query), DEBOUNCE_MS)
    return () => clearTimeout(timer)
  }, [query])

  // Execute search when debounced query changes
  useEffect(() => {
    if (!debouncedQuery.trim()) {
      setResults(EMPTY_RESULTS)
      setLoading(false)
      return
    }

    // Abort any in-flight request
    abortRef.current?.abort()
    abortRef.current = new AbortController()

    const q = debouncedQuery.trim()

    // Filter quick actions client-side
    const filteredActions = QUICK_ACTIONS.map((action) => ({
      action,
      score: Math.max(scoreMatch(action.title, q), scoreMatch(action.subtitle, q)),
    }))
      .filter(({ score }) => score > 0)
      .sort((a, b) => b.score - a.score)
      .slice(0, MAX_PER_CATEGORY)
      .map(({ action }) => action)

    setLoading(true)

    async function doSearch() {
      try {
        const [agentsResult, notifResult] = await Promise.allSettled([
          orchestratorClient.listAgents({ limit: 200 }),
          notifyClient.listNotifications({ limit: 200 }),
        ])

        // --- Agents ---
        type ScoredResult = SearchResult & { _score: number }
        const agentResults: ScoredResult[] = []
        if (agentsResult.status === 'fulfilled') {
          for (const agent of agentsResult.value.items) {
            const score = scoreMatch(agent.name, q)
            if (score > 0) {
              agentResults.push({
                id: `agent-${agent.id}`,
                category: 'agent',
                title: agent.name,
                subtitle: `Status: ${agent.status}`,
                href: `/agents/${agent.id}`,
                _score: score,
              })
            }
          }
          agentResults.sort((a, b) => b._score - a._score)
        }

        // --- Notifications ---
        const notifResults: ScoredResult[] = []
        if (notifResult.status === 'fulfilled') {
          for (const notif of notifResult.value.items) {
            const score = Math.max(
              scoreMatch(notif.title, q),
              scoreMatch(notif.message, q),
            )
            if (score > 0) {
              notifResults.push({
                id: `notif-${notif.id}`,
                category: 'notification',
                title: notif.title,
                subtitle: `${notif.priority} — ${notif.status}`,
                href: `/notifications/${notif.id}`,
                _score: score,
              })
            }
          }
          notifResults.sort((a, b) => b._score - a._score)
        }

        const agents = agentResults.slice(0, MAX_PER_CATEGORY)
        const notifications = notifResults.slice(0, MAX_PER_CATEGORY)

        setResults({
          actions: filteredActions,
          agents,
          notifications,
          total: filteredActions.length + agents.length + notifications.length,
        })
      } catch (err) {
        if (err instanceof Error && err.name === 'AbortError') return
        // On network error still show quick actions
        setResults({ ...EMPTY_RESULTS, actions: filteredActions, total: filteredActions.length })
      } finally {
        setLoading(false)
      }
    }

    void doSearch()
  }, [debouncedQuery])

  const addRecentSearch = useCallback((q: string) => {
    if (!q.trim()) return
    setRecentSearches((prev) => {
      const filtered = prev.filter((s) => s !== q)
      const next = [q, ...filtered].slice(0, MAX_RECENT)
      saveRecentSearches(next)
      return next
    })
  }, [])

  const clearRecentSearches = useCallback(() => {
    setRecentSearches([])
    saveRecentSearches([])
  }, [])

  return {
    query,
    setQuery,
    results,
    loading,
    recentSearches,
    addRecentSearch,
    clearRecentSearches,
  }
}
