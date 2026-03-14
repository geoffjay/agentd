/**
 * CreateMemoryDialog — modal form for creating a new memory record.
 *
 * Fields:
 * - Content (required) — multiline textarea
 * - Memory type — dropdown (Information, Question, Request)
 * - Tags — comma-separated input
 * - Visibility — dropdown (Public, Shared, Private)
 * - Shared with — text input (only shown when visibility is "Shared")
 * - Created by — text input (required)
 *
 * Follows the WorkflowForm dialog pattern with:
 * - Focus trap and ESC to close
 * - Client-side validation
 * - Success/error toast notifications
 */

import { useEffect, useRef, useState } from 'react'
import { X } from 'lucide-react'
import { FocusTrap } from '@/components/common/FocusTrap'
import { useToast } from '@/hooks/useToast'
import type { CreateMemoryRequest, MemoryType, VisibilityLevel } from '@/types/memory'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface CreateMemoryDialogProps {
  open: boolean
  onSave: (request: CreateMemoryRequest) => Promise<unknown>
  onClose: () => void
}

interface FormErrors {
  content?: string
  created_by?: string
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const TYPE_OPTIONS: Array<{ value: MemoryType; label: string }> = [
  { value: 'information', label: 'Information' },
  { value: 'question', label: 'Question' },
  { value: 'request', label: 'Request' },
]

const VISIBILITY_OPTIONS: Array<{ value: VisibilityLevel; label: string }> = [
  { value: 'public', label: 'Public' },
  { value: 'shared', label: 'Shared' },
  { value: 'private', label: 'Private' },
]

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function CreateMemoryDialog({ open, onSave, onClose }: CreateMemoryDialogProps) {
  const contentRef = useRef<HTMLTextAreaElement>(null)
  const toast = useToast()

  // Form state
  const [content, setContent] = useState('')
  const [memoryType, setMemoryType] = useState<MemoryType>('information')
  const [tagsRaw, setTagsRaw] = useState('')
  const [visibility, setVisibility] = useState<VisibilityLevel>('public')
  const [sharedWithRaw, setSharedWithRaw] = useState('')
  const [createdBy, setCreatedBy] = useState('')
  const [errors, setErrors] = useState<FormErrors>({})
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | undefined>()

  // Reset form when dialog opens/closes
  useEffect(() => {
    if (!open) return
    setContent('')
    setMemoryType('information')
    setTagsRaw('')
    setVisibility('public')
    setSharedWithRaw('')
    setCreatedBy('')
    setErrors({})
    setSaveError(undefined)
    setSaving(false)
    // Focus the textarea after render
    setTimeout(() => contentRef.current?.focus(), 50)
  }, [open])

  if (!open) return null

  // Validation
  function validate(): FormErrors {
    const e: FormErrors = {}
    if (!content.trim()) e.content = 'Content is required'
    if (!createdBy.trim()) e.created_by = 'Created by is required'
    return e
  }

  // Submit
  async function handleSave() {
    const e = validate()
    if (Object.keys(e).length > 0) {
      setErrors(e)
      return
    }

    setSaving(true)
    setSaveError(undefined)
    try {
      const tags = tagsRaw
        .split(',')
        .map((t) => t.trim())
        .filter(Boolean)

      const sharedWith = visibility === 'shared'
        ? sharedWithRaw
            .split(',')
            .map((s) => s.trim())
            .filter(Boolean)
        : undefined

      const request: CreateMemoryRequest = {
        content: content.trim(),
        created_by: createdBy.trim(),
        type: memoryType,
        tags: tags.length > 0 ? tags : undefined,
        visibility,
        shared_with: sharedWith,
      }

      await onSave(request)
      toast.success('Memory created')
      onClose()
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : 'Failed to create memory')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/50"
        onClick={onClose}
        aria-hidden="true"
      />

      {/* Focus-trapped dialog panel */}
      <FocusTrap active onEscape={onClose}>
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="create-memory-title"
          className="relative z-10 w-full max-w-lg max-h-[90vh] overflow-y-auto rounded-xl bg-white dark:bg-gray-900 shadow-xl"
        >
          {/* Header */}
          <div className="sticky top-0 z-10 flex items-center justify-between border-b border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 px-6 py-4">
            <h2 id="create-memory-title" className="text-lg font-semibold text-gray-900 dark:text-white">
              Create Memory
            </h2>
            <button
              type="button"
              onClick={onClose}
              className="rounded p-1 text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500"
              aria-label="Close dialog"
            >
              <X size={18} />
            </button>
          </div>

          {/* Body */}
          <div className="px-6 py-5 space-y-4">
            {saveError && (
              <p className="rounded-md bg-red-50 dark:bg-red-900/20 px-3 py-2 text-sm text-red-700 dark:text-red-400">
                {saveError}
              </p>
            )}

            {/* Content */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Content <span className="text-red-500">*</span>
              </label>
              <textarea
                ref={contentRef}
                value={content}
                onChange={(e) => setContent(e.target.value)}
                placeholder="Enter the memory content…"
                rows={4}
                className={fieldClass(errors.content)}
              />
              {errors.content && <p className="mt-1 text-xs text-red-500">{errors.content}</p>}
            </div>

            {/* Created by */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Created by <span className="text-red-500">*</span>
              </label>
              <input
                type="text"
                value={createdBy}
                onChange={(e) => setCreatedBy(e.target.value)}
                placeholder="e.g. agent-1 or user@example.com"
                className={fieldClass(errors.created_by)}
              />
              {errors.created_by && <p className="mt-1 text-xs text-red-500">{errors.created_by}</p>}
            </div>

            {/* Type + Visibility row */}
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Type
                </label>
                <select
                  value={memoryType}
                  onChange={(e) => setMemoryType(e.target.value as MemoryType)}
                  className={fieldClass()}
                >
                  {TYPE_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Visibility
                </label>
                <select
                  value={visibility}
                  onChange={(e) => setVisibility(e.target.value as VisibilityLevel)}
                  className={fieldClass()}
                >
                  {VISIBILITY_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            {/* Shared with (conditional) */}
            {visibility === 'shared' && (
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Shared with <span className="text-gray-400">(comma-separated)</span>
                </label>
                <input
                  type="text"
                  value={sharedWithRaw}
                  onChange={(e) => setSharedWithRaw(e.target.value)}
                  placeholder="e.g. agent-2, user@example.com"
                  className={fieldClass()}
                />
              </div>
            )}

            {/* Tags */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Tags <span className="text-gray-400">(comma-separated)</span>
              </label>
              <input
                type="text"
                value={tagsRaw}
                onChange={(e) => setTagsRaw(e.target.value)}
                placeholder="e.g. deployment, api, critical"
                className={fieldClass()}
              />
              {/* Tag preview chips */}
              {tagsRaw.trim() && (
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {tagsRaw
                    .split(',')
                    .map((t) => t.trim())
                    .filter(Boolean)
                    .map((tag, i) => (
                      <span
                        key={`${tag}-${i}`}
                        className="rounded-full bg-gray-200 dark:bg-gray-700 px-2 py-0.5 text-[11px] font-medium text-gray-700 dark:text-gray-300"
                      >
                        {tag}
                      </span>
                    ))}
                </div>
              )}
            </div>
          </div>

          {/* Footer */}
          <div className="sticky bottom-0 flex items-center justify-end gap-3 border-t border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 px-6 py-4">
            <button
              type="button"
              onClick={onClose}
              disabled={saving}
              className="rounded-md border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={handleSave}
              disabled={saving}
              className="rounded-md bg-primary-600 px-4 py-2 text-sm font-medium text-white hover:bg-primary-700 transition-colors disabled:opacity-50"
            >
              {saving ? 'Creating…' : 'Create memory'}
            </button>
          </div>
        </div>
      </FocusTrap>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Style helpers
// ---------------------------------------------------------------------------

function fieldClass(error?: string, extra = ''): string {
  return [
    'w-full rounded-md border px-3 py-2 text-sm',
    'bg-white dark:bg-gray-900',
    'text-gray-900 dark:text-white',
    'focus:outline-none focus:ring-2 focus:ring-primary-500',
    'disabled:cursor-not-allowed disabled:opacity-50',
    error
      ? 'border-red-400 dark:border-red-500'
      : 'border-gray-300 dark:border-gray-600',
    extra,
  ]
    .filter(Boolean)
    .join(' ')
}

export default CreateMemoryDialog
