/**
 * MemoryCard — displays a single memory record with:
 * - Content preview with expand/collapse
 * - MemoryType badge with colour coding
 * - VisibilityLevel badge
 * - Tags displayed as chips
 * - Created by / created at metadata
 * - Action buttons: Edit visibility, Delete (with confirmation)
 */

import { useState } from 'react'
import {
  ChevronDown,
  ChevronRight,
  Clock,
  Eye,
  Globe,
  Lock,
  Trash2,
  Users,
} from 'lucide-react'
import type { Memory } from '@/types/memory'
import type { VisibilityLevel } from '@/types/memory'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Max characters to show before truncating. */
const CONTENT_PREVIEW_LENGTH = 180

const TYPE_STYLES: Record<string, string> = {
  information: 'bg-blue-900/50 text-blue-300',
  question: 'bg-amber-900/50 text-amber-300',
  request: 'bg-purple-900/50 text-purple-300',
}

const TYPE_LABELS: Record<string, string> = {
  information: 'Information',
  question: 'Question',
  request: 'Request',
}

const VISIBILITY_STYLES: Record<string, string> = {
  public: 'bg-green-900/50 text-green-300',
  shared: 'bg-yellow-900/50 text-yellow-300',
  private: 'bg-red-900/50 text-red-300',
}

const VISIBILITY_ICONS: Record<VisibilityLevel, React.ReactNode> = {
  public: <Globe size={10} aria-hidden="true" />,
  shared: <Users size={10} aria-hidden="true" />,
  private: <Lock size={10} aria-hidden="true" />,
}

function formatRelativeTime(dateStr: string): string {
  const diffMs = Date.now() - new Date(dateStr).getTime()
  const diffSec = Math.floor(diffMs / 1000)
  if (diffSec < 60) return 'just now'
  const diffMin = Math.floor(diffSec / 60)
  if (diffMin < 60) return `${diffMin}m ago`
  const diffHour = Math.floor(diffMin / 60)
  if (diffHour < 24) return `${diffHour}h ago`
  const diffDay = Math.floor(diffHour / 24)
  return `${diffDay}d ago`
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface MemoryCardProps {
  memory: Memory
  onEditVisibility: (memory: Memory) => void
  onDelete: (id: string) => void
}

export function MemoryCard({ memory, onEditVisibility, onDelete }: MemoryCardProps) {
  const [expanded, setExpanded] = useState(false)

  const isLong = memory.content.length > CONTENT_PREVIEW_LENGTH
  const displayContent = expanded || !isLong
    ? memory.content
    : memory.content.slice(0, CONTENT_PREVIEW_LENGTH) + '…'

  return (
    <article
      aria-label={`Memory: ${memory.content.slice(0, 60)}`}
      className="rounded-lg border border-gray-700 bg-gray-800 transition-colors hover:border-gray-600"
    >
      <div className="p-4">
        {/* Top row: badges */}
        <div className="flex flex-wrap items-start gap-2">
          {/* Type badge */}
          <span
            className={[
              'shrink-0 rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide',
              TYPE_STYLES[memory.type] ?? 'bg-gray-700 text-gray-300',
            ].join(' ')}
          >
            {TYPE_LABELS[memory.type] ?? memory.type}
          </span>

          {/* Visibility badge */}
          <span
            className={[
              'shrink-0 inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide',
              VISIBILITY_STYLES[memory.visibility] ?? 'bg-gray-700 text-gray-300',
            ].join(' ')}
          >
            {VISIBILITY_ICONS[memory.visibility]}
            {memory.visibility}
          </span>

          <div className="flex-1" />

          {/* Timestamp */}
          <span className="flex items-center gap-1 text-xs text-gray-400 shrink-0">
            <Clock size={11} aria-hidden="true" />
            {formatRelativeTime(memory.created_at)}
          </span>
        </div>

        {/* Content */}
        <div className="mt-2">
          <p className="text-sm text-gray-300 whitespace-pre-wrap leading-relaxed">
            {displayContent}
          </p>

          {/* Expand/collapse toggle */}
          {isLong && (
            <button
              type="button"
              aria-expanded={expanded}
              onClick={() => setExpanded((v) => !v)}
              className="mt-1 flex items-center gap-1 text-xs text-gray-400 hover:text-gray-300"
            >
              {expanded ? (
                <ChevronDown size={12} aria-hidden="true" />
              ) : (
                <ChevronRight size={12} aria-hidden="true" />
              )}
              {expanded ? 'Show less' : 'Show more'}
            </button>
          )}
        </div>

        {/* Tags */}
        {memory.tags.length > 0 && (
          <div className="mt-2 flex flex-wrap gap-1.5">
            {memory.tags.map((tag) => (
              <span
                key={tag}
                className="rounded-full bg-gray-700 px-2 py-0.5 text-[11px] font-medium text-gray-300"
              >
                {tag}
              </span>
            ))}
          </div>
        )}

        {/* Shared with */}
        {memory.visibility === 'shared' && memory.shared_with.length > 0 && (
          <div className="mt-2 text-xs text-gray-500">
            Shared with: {memory.shared_with.join(', ')}
          </div>
        )}

        {/* Meta row + actions */}
        <div className="mt-3 flex flex-wrap items-center gap-3">
          {/* Creator */}
          <span className="text-xs text-gray-500">
            by <span className="text-gray-400">{memory.created_by}</span>
          </span>

          {/* References count */}
          {memory.references.length > 0 && (
            <span className="text-xs text-gray-500">
              {memory.references.length} ref{memory.references.length !== 1 ? 's' : ''}
            </span>
          )}

          <div className="flex-1" />

          {/* Action buttons */}
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => onEditVisibility(memory)}
              aria-label="Edit visibility"
              className="rounded px-2.5 py-1 text-xs font-medium bg-gray-700 text-gray-300 hover:bg-gray-600 transition-colors flex items-center gap-1"
            >
              <Eye size={12} aria-hidden="true" />
              Visibility
            </button>

            <button
              type="button"
              onClick={() => onDelete(memory.id)}
              aria-label="Delete memory"
              className="rounded px-2.5 py-1 text-xs font-medium bg-red-900/40 text-red-400 hover:bg-red-900/70 transition-colors flex items-center gap-1"
            >
              <Trash2 size={12} aria-hidden="true" />
              Delete
            </button>
          </div>
        </div>
      </div>
    </article>
  )
}

export default MemoryCard
