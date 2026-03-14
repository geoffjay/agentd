/**
 * Client for the Memory service (default port 17008).
 *
 * Provides strongly-typed methods for all memory operations including
 * creating, listing, searching, and deleting memories.
 */

import { ApiClient } from './base'
import { serviceConfig } from './config'
import type { HealthResponse, PaginatedResponse } from '@/types/common'
import type {
  CreateMemoryRequest,
  DeleteResponse,
  Memory,
  MemoryListParams,
  SearchRequest,
  SearchResponse,
  UpdateVisibilityRequest,
} from '@/types/memory'

export class MemoryClient extends ApiClient {
  // -------------------------------------------------------------------------
  // Health
  // -------------------------------------------------------------------------

  /** `GET /health` — service health check. */
  getHealth(): Promise<HealthResponse> {
    return this.get<HealthResponse>('/health')
  }

  // -------------------------------------------------------------------------
  // Memories – CRUD
  // -------------------------------------------------------------------------

  /** `GET /memories` — list memories with optional filters. */
  listMemories(params?: MemoryListParams): Promise<PaginatedResponse<Memory>> {
    return this.get<PaginatedResponse<Memory>>(
      '/memories',
      params as Record<string, string | number | boolean | undefined>,
    )
  }

  /** `GET /memories/:id` — retrieve a single memory by ID. */
  getMemory(id: string): Promise<Memory> {
    return this.get<Memory>(`/memories/${id}`)
  }

  /** `POST /memories` — create a new memory record. */
  createMemory(request: CreateMemoryRequest): Promise<Memory> {
    return this.post<Memory>('/memories', request)
  }

  /** `DELETE /memories/:id` — delete a memory. */
  deleteMemory(id: string): Promise<DeleteResponse> {
    return this.delete<DeleteResponse>(`/memories/${id}`)
  }

  // -------------------------------------------------------------------------
  // Visibility
  // -------------------------------------------------------------------------

  /** `PUT /memories/:id/visibility` — update visibility and share list. */
  updateVisibility(id: string, request: UpdateVisibilityRequest): Promise<Memory> {
    return this.put<Memory>(`/memories/${id}/visibility`, request)
  }

  // -------------------------------------------------------------------------
  // Search
  // -------------------------------------------------------------------------

  /** `POST /memories/search` — semantic similarity search. */
  searchMemories(request: SearchRequest): Promise<SearchResponse> {
    return this.post<SearchResponse>('/memories/search', request)
  }
}

/** Singleton client instance using the configured service URL */
export const memoryClient = new MemoryClient({
  baseUrl: serviceConfig.memoryServiceUrl,
})
