/**
 * Client for the Communicate service (default port 17010).
 *
 * Provides strongly-typed methods for all communicate operations including
 * rooms, participants, messages, and WebSocket connections.
 */

import { ApiClient } from './base'
import { serviceConfig } from './config'
import type { HealthResponse, PaginatedResponse } from '@/types/common'
import type {
  AddParticipantRequest,
  ChatMessage,
  CreateMessageRequest,
  CreateRoomRequest,
  ListMessagesParams,
  ListRoomsParams,
  Participant,
  Room,
  UpdateRoomRequest,
} from '@/types/communicate'

export class CommunicateClient extends ApiClient {
  // -------------------------------------------------------------------------
  // Health
  // -------------------------------------------------------------------------

  /** `GET /health` — service health check. */
  getHealth(): Promise<HealthResponse> {
    return this.get<HealthResponse>('/health')
  }

  // -------------------------------------------------------------------------
  // Rooms
  // -------------------------------------------------------------------------

  /** `GET /rooms` — list rooms with optional filters. */
  listRooms(params?: ListRoomsParams): Promise<PaginatedResponse<Room>> {
    return this.get<PaginatedResponse<Room>>(
      '/rooms',
      params as Record<string, string | number | boolean | undefined>,
    )
  }

  /** `GET /rooms/:id` — get a single room by ID. */
  getRoom(id: string): Promise<Room> {
    return this.get<Room>(`/rooms/${id}`)
  }

  /** `POST /rooms` — create a new room. */
  createRoom(request: CreateRoomRequest): Promise<Room> {
    return this.post<Room>('/rooms', request)
  }

  /** `PUT /rooms/:id` — update a room's topic/description. */
  updateRoom(id: string, request: UpdateRoomRequest): Promise<Room> {
    return this.put<Room>(`/rooms/${id}`, request)
  }

  /** `DELETE /rooms/:id` — delete a room. */
  deleteRoom(id: string): Promise<void> {
    return this.delete<void>(`/rooms/${id}`)
  }

  // -------------------------------------------------------------------------
  // Participants
  // -------------------------------------------------------------------------

  /** `GET /rooms/:id/participants` — list participants in a room. */
  listParticipants(roomId: string, params?: { limit?: number; offset?: number }): Promise<PaginatedResponse<Participant>> {
    return this.get<PaginatedResponse<Participant>>(
      `/rooms/${roomId}/participants`,
      params as Record<string, string | number | boolean | undefined>,
    )
  }

  /** `POST /rooms/:id/participants` — add a participant to a room. */
  addParticipant(roomId: string, request: AddParticipantRequest): Promise<Participant> {
    return this.post<Participant>(`/rooms/${roomId}/participants`, request)
  }

  /** `DELETE /rooms/:id/participants/:identifier` — remove a participant from a room. */
  removeParticipant(roomId: string, identifier: string): Promise<void> {
    return this.delete<void>(`/rooms/${roomId}/participants/${encodeURIComponent(identifier)}`)
  }

  /** `GET /participants/:identifier/rooms` — list rooms for a participant. */
  listRoomsForParticipant(identifier: string, params?: { limit?: number; offset?: number }): Promise<PaginatedResponse<Room>> {
    return this.get<PaginatedResponse<Room>>(
      `/participants/${encodeURIComponent(identifier)}/rooms`,
      params as Record<string, string | number | boolean | undefined>,
    )
  }

  // -------------------------------------------------------------------------
  // Messages
  // -------------------------------------------------------------------------

  /** `GET /rooms/:id/messages/latest` — get N most recent messages (default 50). */
  getLatestMessages(roomId: string, count = 50): Promise<ChatMessage[]> {
    return this.get<ChatMessage[]>(`/rooms/${roomId}/messages/latest`, { count })
  }

  /** `GET /rooms/:id/messages` — list messages with pagination. */
  listMessages(roomId: string, params?: ListMessagesParams): Promise<PaginatedResponse<ChatMessage>> {
    return this.get<PaginatedResponse<ChatMessage>>(
      `/rooms/${roomId}/messages`,
      params as Record<string, string | number | boolean | undefined>,
    )
  }

  /** `POST /rooms/:id/messages` — send a message to a room. */
  sendMessage(roomId: string, request: CreateMessageRequest): Promise<ChatMessage> {
    return this.post<ChatMessage>(`/rooms/${roomId}/messages`, request)
  }

  /** `DELETE /messages/:id` — delete a message. */
  deleteMessage(id: string): Promise<void> {
    return this.delete<void>(`/messages/${id}`)
  }

  // -------------------------------------------------------------------------
  // WebSocket
  // -------------------------------------------------------------------------

  /**
   * Open a WebSocket connection to the communicate service.
   * The caller must send a JSON `{ type: "subscribe", room_id }` command
   * to start receiving room events.
   */
  openWebSocket(): WebSocket {
    return super.openWebSocket('/ws')
  }
}

/** Singleton client instance using the configured service URL. */
export const communicateClient = new CommunicateClient({
  baseUrl: serviceConfig.communicateServiceUrl,
})
