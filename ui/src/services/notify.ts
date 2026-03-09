/**
 * Client for the Notify service (default port 17004).
 *
 * Manages notification CRUD with status filtering.
 */

import { ApiClient } from './base'
import { serviceConfig } from './config'
import type { HealthResponse, PaginatedResponse } from '@/types/common'
import type {
  CountResponse,
  CreateNotificationRequest,
  ListNotificationsParams,
  Notification,
  UpdateNotificationRequest,
} from '@/types/notify'

export class NotifyClient extends ApiClient {
  // -------------------------------------------------------------------------
  // Health
  // -------------------------------------------------------------------------

  getHealth(): Promise<HealthResponse> {
    return this.get<HealthResponse>('/health')
  }

  // -------------------------------------------------------------------------
  // Notifications – CRUD
  // -------------------------------------------------------------------------

  listNotifications(params?: ListNotificationsParams): Promise<PaginatedResponse<Notification>> {
    return this.get<PaginatedResponse<Notification>>(
      '/notifications',
      params as Record<string, string>,
    )
  }

  createNotification(request: CreateNotificationRequest): Promise<Notification> {
    return this.post<Notification>('/notifications', request)
  }

  getNotification(id: string): Promise<Notification> {
    return this.get<Notification>(`/notifications/${id}`)
  }

  updateNotification(id: string, request: UpdateNotificationRequest): Promise<Notification> {
    return this.put<Notification>(`/notifications/${id}`, request)
  }

  deleteNotification(id: string): Promise<void> {
    return this.delete<void>(`/notifications/${id}`)
  }

  // -------------------------------------------------------------------------
  // Filtered views
  // -------------------------------------------------------------------------

  /** Returns actionable notifications (Pending or Viewed, not expired) */
  listActionable(params?: ListNotificationsParams): Promise<PaginatedResponse<Notification>> {
    return this.get<PaginatedResponse<Notification>>(
      '/notifications/actionable',
      params as Record<string, string>,
    )
  }

  /** Returns completed/dismissed notification history */
  listHistory(params?: ListNotificationsParams): Promise<PaginatedResponse<Notification>> {
    return this.get<PaginatedResponse<Notification>>(
      '/notifications/history',
      params as Record<string, string>,
    )
  }

  // -------------------------------------------------------------------------
  // Count
  // -------------------------------------------------------------------------

  /** Returns total count and breakdown by status */
  getCount(): Promise<CountResponse> {
    return this.get<CountResponse>('/notifications/count')
  }
}

/** Singleton client instance using the configured service URL */
export const notifyClient = new NotifyClient({
  baseUrl: serviceConfig.notifyServiceUrl,
})
