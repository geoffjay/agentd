/**
 * Test data factory for Notification and related Notify service types.
 *
 * Usage:
 *   const notif = makeNotification()
 *   const notif = makeNotification({ priority: 'Urgent', status: 'Pending' })
 *   const notifs = makeNotificationList(10)
 */

import type { Notification, CountResponse, StatusCount } from '@/types/notify'

let _seq = 0
function nextId(): string {
  return String(++_seq)
}

/** Reset the sequence counter (call in beforeEach to get predictable IDs) */
export function resetNotificationSeq(): void {
  _seq = 0
}

// ---------------------------------------------------------------------------
// Notification factory
// ---------------------------------------------------------------------------

export function makeNotification(overrides?: Partial<Notification>): Notification {
  const id = nextId()
  return {
    id,
    source: 'System',
    lifetime: { type: 'Persistent' },
    priority: 'Normal',
    status: 'Pending',
    title: `Test Notification ${id}`,
    message: `This is test notification message ${id}`,
    requires_response: false,
    created_at: '2024-01-01T00:00:00Z',
    updated_at: '2024-01-01T00:00:00Z',
    ...overrides,
  }
}

/** Urgent notification requiring a response */
export function makeUrgentNotification(overrides?: Partial<Notification>): Notification {
  return makeNotification({
    priority: 'Urgent',
    requires_response: true,
    ...overrides,
  })
}

export function makeNotificationList(
  count: number,
  overrides?: Partial<Notification>,
): Notification[] {
  return Array.from({ length: count }, () => makeNotification(overrides))
}

// ---------------------------------------------------------------------------
// Count response factory
// ---------------------------------------------------------------------------

export function makeStatusCount(status: string, count: number): StatusCount {
  return { status, count }
}

export function makeCountResponse(
  total: number,
  byStatus?: Partial<Record<string, number>>,
): CountResponse {
  const statusCounts = byStatus
    ? Object.entries(byStatus).map(([s, c]) => makeStatusCount(s, c ?? 0))
    : [makeStatusCount('Pending', total)]

  return { total, by_status: statusCounts }
}
