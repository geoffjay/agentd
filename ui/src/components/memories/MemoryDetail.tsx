/**
 * MemoryDetail — drawer content for a single memory record.
 *
 * Shows all memory details including full content, type, visibility,
 * tags, creator, references, and action buttons.
 */

import { Clock, Eye, Globe, Lock, Trash2, Users } from 'lucide-react'
import type { Memory, VisibilityLevel } from '@/types/memory'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const TYPE_STYLES: Record<string, string> = {
  information: 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400',
  question: 'bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400',
  request: 'bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-400',
}

const TYPE_LABELS: Record<string, string> = {
  information: 'Information',
  question: 'Question',
  request: 'Request',
}

const VISIBILITY_STYLES: Record<string, string> = {
  public: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400',
  shared: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400',
  private: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400',
}

const VISIBILITY_ICONS: Record<VisibilityLevel, React.ReactNode> = {
  public: <Globe size={12} aria-hidden="true" />,
  shared: <Users size={12} aria-hidden="true" />,
  private: <Lock size={12} aria-hidden="true" />,
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface MemoryDetailProps {
  memory: Memory
  onEditVisibility: (memory: Memory) => void
  onDelete: (id: string) => void
}

export function MemoryDetail({ memory, onEditVisibility, onDelete }: MemoryDetailProps) {
  return (
    <div className="space-y-5">
      {/* Badges */}
      <div className="flex flex-wrap items-center gap-2">
        <span
          className={[
            'inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium',
            TYPE_STYLES[memory.type] ?? 'bg-gray-100 text-gray-600',
          ].join(' ')}
        >
          {TYPE_LABELS[memory.type] ?? memory.type}
        </span>
        <span
          className={[
            'inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium',
            VISIBILITY_STYLES[memory.visibility] ?? 'bg-gray-100 text-gray-600',
          ].join(' ')}
        >
          {VISIBILITY_ICONS[memory.visibility]}
          {memory.visibility}
        </span>
      </div>

      {/* Content */}
      <div>
        <h3 className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
          Content
        </h3>
        <div className="mt-2 rounded-lg bg-gray-100 p-4 text-sm text-gray-800 whitespace-pre-wrap leading-relaxed dark:bg-gray-800 dark:text-gray-300">
          {memory.content}
        </div>
      </div>

      {/* Tags */}
      {memory.tags.length > 0 && (
        <div>
          <h3 className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
            Tags
          </h3>
          <div className="mt-2 flex flex-wrap gap-1.5">
            {memory.tags.map((tag) => (
              <span
                key={tag}
                className="rounded-full bg-gray-100 px-2.5 py-0.5 text-xs font-medium text-gray-700 dark:bg-gray-800 dark:text-gray-300"
              >
                {tag}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Creator & timestamps */}
      <div className="grid grid-cols-2 gap-4">
        <div>
          <h3 className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
            Created By
          </h3>
          <p className="mt-1 text-sm text-gray-700 dark:text-gray-300">{memory.created_by}</p>
        </div>
        <div>
          <h3 className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
            Created
          </h3>
          <div className="mt-1 flex items-center gap-1.5 text-sm text-gray-700 dark:text-gray-300">
            <Clock size={13} className="text-gray-400" />
            {new Date(memory.created_at).toLocaleString()}
          </div>
        </div>
      </div>

      {/* Shared with */}
      {memory.visibility === 'shared' && memory.shared_with.length > 0 && (
        <div>
          <h3 className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
            Shared With
          </h3>
          <div className="mt-2 flex flex-wrap gap-1.5">
            {memory.shared_with.map((actor) => (
              <span
                key={actor}
                className="rounded-full bg-yellow-50 px-2.5 py-0.5 text-xs font-medium text-yellow-800 dark:bg-yellow-900/20 dark:text-yellow-400"
              >
                {actor}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* References */}
      {memory.references.length > 0 && (
        <div>
          <h3 className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
            References
          </h3>
          <ul className="mt-2 space-y-1">
            {memory.references.map((ref) => (
              <li key={ref} className="text-xs font-mono text-gray-500 dark:text-gray-400">
                {ref}
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* Actions */}
      <div className="border-t border-gray-200 pt-4 dark:border-gray-700">
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => onEditVisibility(memory)}
            className="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium bg-gray-100 text-gray-700 hover:bg-gray-200 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700 transition-colors"
          >
            <Eye size={13} />
            Edit Visibility
          </button>
          <button
            type="button"
            onClick={() => onDelete(memory.id)}
            className="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium bg-red-50 text-red-600 hover:bg-red-100 dark:bg-red-900/20 dark:text-red-400 dark:hover:bg-red-900/40 transition-colors"
          >
            <Trash2 size={13} />
            Delete
          </button>
        </div>
      </div>
    </div>
  )
}

export default MemoryDetail
