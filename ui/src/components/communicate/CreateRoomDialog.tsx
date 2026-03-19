/**
 * CreateRoomDialog — modal form for creating a new communicate room.
 *
 * Fields:
 * - Name (required)
 * - Type — group | direct | broadcast
 * - Topic (optional)
 * - Description (optional)
 *
 * Calls communicateClient.createRoom and invokes onCreated on success.
 */

import { useEffect, useRef, useState } from 'react'
import { X } from 'lucide-react'
import { FocusTrap } from '@/components/common/FocusTrap'
import { communicateClient } from '@/services/communicate'
import type { Room, RoomType } from '@/types/communicate'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface CreateRoomDialogProps {
  open: boolean
  /** The identifier of the person creating the room. */
  createdBy: string
  onCreated: (room: Room) => void
  onClose: () => void
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const ROOM_TYPE_OPTIONS: Array<{ value: RoomType; label: string; description: string }> = [
  { value: 'group', label: 'Group', description: 'Open group conversation' },
  { value: 'direct', label: 'Direct', description: 'Private 1-on-1 or small group' },
  { value: 'broadcast', label: 'Broadcast', description: 'One-way announcements' },
]

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function CreateRoomDialog({
  open,
  createdBy,
  onCreated,
  onClose,
}: CreateRoomDialogProps) {
  const nameRef = useRef<HTMLInputElement>(null)

  const [name, setName] = useState('')
  const [roomType, setRoomType] = useState<RoomType>('group')
  const [topic, setTopic] = useState('')
  const [description, setDescription] = useState('')
  const [nameError, setNameError] = useState<string | undefined>()
  const [saveError, setSaveError] = useState<string | undefined>()
  const [saving, setSaving] = useState(false)

  // Reset form when dialog opens
  useEffect(() => {
    if (!open) return
    setName('')
    setRoomType('group')
    setTopic('')
    setDescription('')
    setNameError(undefined)
    setSaveError(undefined)
    setSaving(false)
    setTimeout(() => nameRef.current?.focus(), 50)
  }, [open])

  if (!open) return null

  async function handleCreate() {
    if (!name.trim()) {
      setNameError('Room name is required')
      return
    }
    setNameError(undefined)
    setSaveError(undefined)
    setSaving(true)
    try {
      const room = await communicateClient.createRoom({
        name: name.trim(),
        room_type: roomType,
        topic: topic.trim() || undefined,
        description: description.trim() || undefined,
        created_by: createdBy,
      })
      onCreated(room)
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : 'Failed to create room')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-black/50" onClick={onClose} aria-hidden="true" />
      <FocusTrap active onEscape={onClose}>
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="create-room-title"
          className="relative z-10 w-full max-w-md rounded-xl bg-gray-800 shadow-xl border border-gray-700"
        >
          {/* Header */}
          <div className="flex items-center justify-between border-b border-gray-700 px-6 py-4">
            <h2 id="create-room-title" className="text-base font-semibold text-white">
              Create Room
            </h2>
            <button
              type="button"
              onClick={onClose}
              aria-label="Close dialog"
              className="rounded p-1 text-gray-400 hover:text-gray-200 transition-colors"
            >
              <X size={18} />
            </button>
          </div>

          {/* Body */}
          <div className="px-6 py-5 space-y-4">
            {saveError && (
              <p className="rounded-md bg-red-900/30 border border-red-700 px-3 py-2 text-sm text-red-400">
                {saveError}
              </p>
            )}

            {/* Name */}
            <div>
              <label className="block text-sm font-medium text-gray-300 mb-1">
                Name <span className="text-red-400">*</span>
              </label>
              <input
                ref={nameRef}
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. general, ops-team"
                className={fieldClass(nameError)}
              />
              {nameError && <p className="mt-1 text-xs text-red-400">{nameError}</p>}
            </div>

            {/* Room type */}
            <div>
              <label className="block text-sm font-medium text-gray-300 mb-1">Type</label>
              <div className="grid grid-cols-3 gap-2">
                {ROOM_TYPE_OPTIONS.map((opt) => (
                  <button
                    key={opt.value}
                    type="button"
                    onClick={() => setRoomType(opt.value)}
                    className={[
                      'flex flex-col items-start rounded-md border px-3 py-2 text-left text-xs transition-colors',
                      roomType === opt.value
                        ? 'border-primary-500 bg-primary-900/30 text-primary-300'
                        : 'border-gray-600 bg-gray-700 text-gray-300 hover:border-gray-500',
                    ].join(' ')}
                  >
                    <span className="font-medium">{opt.label}</span>
                    <span className="text-[10px] text-gray-500 mt-0.5">{opt.description}</span>
                  </button>
                ))}
              </div>
            </div>

            {/* Topic */}
            <div>
              <label className="block text-sm font-medium text-gray-300 mb-1">
                Topic <span className="text-gray-500 font-normal">(optional)</span>
              </label>
              <input
                type="text"
                value={topic}
                onChange={(e) => setTopic(e.target.value)}
                placeholder="e.g. Project discussions and updates"
                className={fieldClass()}
              />
            </div>

            {/* Description */}
            <div>
              <label className="block text-sm font-medium text-gray-300 mb-1">
                Description <span className="text-gray-500 font-normal">(optional)</span>
              </label>
              <textarea
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="What is this room for?"
                rows={2}
                className={fieldClass() + ' resize-none'}
              />
            </div>
          </div>

          {/* Footer */}
          <div className="flex justify-end gap-3 border-t border-gray-700 px-6 py-4">
            <button
              type="button"
              onClick={onClose}
              disabled={saving}
              className="rounded-md border border-gray-600 px-4 py-2 text-sm font-medium text-gray-300 hover:bg-gray-700 transition-colors disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={() => void handleCreate()}
              disabled={saving}
              className="rounded-md bg-primary-600 px-4 py-2 text-sm font-medium text-white hover:bg-primary-700 transition-colors disabled:opacity-50"
            >
              {saving ? 'Creating…' : 'Create room'}
            </button>
          </div>
        </div>
      </FocusTrap>
    </div>
  )
}

function fieldClass(error?: string): string {
  return [
    'w-full rounded-md border px-3 py-2 text-sm bg-gray-900 text-white',
    'placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-primary-500',
    'disabled:opacity-50',
    error ? 'border-red-500' : 'border-gray-600',
  ].join(' ')
}
