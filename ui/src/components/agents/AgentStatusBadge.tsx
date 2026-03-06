/**
 * AgentStatusBadge — status indicator for agent lifecycle states.
 *
 * Wraps the generic StatusBadge, keeping agent-specific display logic here.
 * Supports both 'badge' (pill with text) and 'dot' (coloured circle) variants.
 */

import { StatusBadge } from '@/components/common/StatusBadge'
import type { AgentStatus } from '@/types/orchestrator'

export interface AgentStatusBadgeProps {
  status: AgentStatus
  variant?: 'badge' | 'dot'
  className?: string
}

export function AgentStatusBadge({
  status,
  variant = 'badge',
  className,
}: AgentStatusBadgeProps) {
  return <StatusBadge status={status} variant={variant} className={className} />
}

export default AgentStatusBadge
