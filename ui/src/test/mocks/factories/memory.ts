/**
 * Test data factory for Memory and related Memory service types.
 *
 * Usage:
 *   const mem = makeMemory()
 *   const mem = makeMemory({ type: 'question', visibility: 'private' })
 *   const mems = makeMemoryList(5)
 *   const resp = makeSearchResponse([mem1, mem2])
 */

import type {
  Memory,
  MemoryType,
  VisibilityLevel,
  SearchResponse,
  DeleteResponse,
} from '@/types/memory'

let _seq = 0
function nextId(): string {
  const id = ++_seq
  const ts = 1705312800000 + id * 1000 // deterministic timestamps starting 2024-01-15
  const prefix = id.toString(16).padStart(8, '0')
  return `mem_${ts}_${prefix}`
}

/** Reset the sequence counter (call in beforeEach to get predictable IDs) */
export function resetMemorySeq(): void {
  _seq = 0
}

// ---------------------------------------------------------------------------
// Memory factory
// ---------------------------------------------------------------------------

export function makeMemory(overrides?: Partial<Memory>): Memory {
  const id = overrides?.id ?? nextId()
  const seq = _seq
  return {
    id,
    content: `Test memory content ${seq}`,
    type: 'information' as MemoryType,
    tags: ['test'],
    created_by: 'agent-1',
    owner: undefined,
    created_at: '2024-01-15T10:00:00.000Z',
    updated_at: '2024-01-15T10:00:00.000Z',
    visibility: 'public' as VisibilityLevel,
    shared_with: [],
    references: [],
    ...overrides,
  }
}

/** Create a list of N memories with auto-incrementing IDs */
export function makeMemoryList(count: number, overrides?: Partial<Memory>): Memory[] {
  return Array.from({ length: count }, () => makeMemory(overrides))
}

/** Create a memory with question type */
export function makeQuestionMemory(overrides?: Partial<Memory>): Memory {
  return makeMemory({
    type: 'question',
    content: 'What is the deployment process?',
    tags: ['devops', 'question'],
    ...overrides,
  })
}

/** Create a memory with request type */
export function makeRequestMemory(overrides?: Partial<Memory>): Memory {
  return makeMemory({
    type: 'request',
    content: 'Please review this code change.',
    tags: ['review', 'request'],
    ...overrides,
  })
}

/** Create a private memory */
export function makePrivateMemory(overrides?: Partial<Memory>): Memory {
  return makeMemory({
    visibility: 'private',
    ...overrides,
  })
}

/** Create a shared memory */
export function makeSharedMemory(overrides?: Partial<Memory>): Memory {
  return makeMemory({
    visibility: 'shared',
    shared_with: ['agent-2', 'agent-3'],
    ...overrides,
  })
}

// ---------------------------------------------------------------------------
// Response factories
// ---------------------------------------------------------------------------

/** Create a search response with the given memories */
export function makeSearchResponse(
  memories?: Memory[],
  total?: number,
): SearchResponse {
  const mems = memories ?? makeMemoryList(2)
  return {
    memories: mems,
    total: total ?? mems.length,
  }
}

/** Create a delete response */
export function makeDeleteResponse(deleted = true): DeleteResponse {
  return { deleted }
}
