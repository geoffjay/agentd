/**
 * RoomSettingsPanel — in-panel UI for managing a communicate room.
 *
 * Shown in the right column when the settings icon is clicked.
 *
 * Features:
 * - View room info (type, created by, created at)
 * - Edit topic and description
 * - Add participants (identifier, kind, display name)
 * - Remove participants
 * - Delete room (with confirmation dialog)
 * - Leave room for the local human participant
 */

import { useEffect, useState, useCallback } from 'react'
import { X, Trash2, UserPlus, UserMinus, Save } from 'lucide-react'
import { ConfirmDialog } from '@/components/common/ConfirmDialog'
import { communicateClient } from '@/services/communicate'
import { mapApiError } from '@/hooks/useToast'
import type { Participant, ParticipantKind, ParticipantRole, Room } from '@/types/communicate'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface RoomSettingsPanelProps {
  room: Room
  localIdentifier: string
  onClose: () => void
  onRoomDeleted: () => void
  onLeft: () => void
  onRoomUpdated: (updated: Room) => void
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function RoomSettingsPanel({
  room,
  localIdentifier,
  onClose,
  onRoomDeleted,
  onLeft,
  onRoomUpdated,
}: RoomSettingsPanelProps) {
  // Editable fields
  const [topic, setTopic] = useState(room.topic ?? '')
  const [description, setDescription] = useState(room.description ?? '')
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | undefined>()

  // Participants
  const [participants, setParticipants] = useState<Participant[]>([])
  const [participantsLoading, setParticipantsLoading] = useState(false)

  // Add participant form
  const [addIdentifier, setAddIdentifier] = useState('')
  const [addDisplayName, setAddDisplayName] = useState('')
  const [addKind, setAddKind] = useState<ParticipantKind>('human')
  const [addRole, setAddRole] = useState<ParticipantRole>('member')
  const [addError, setAddError] = useState<string | undefined>()
  const [adding, setAdding] = useState(false)

  // Confirm dialogs
  const [showDeleteRoom, setShowDeleteRoom] = useState(false)
  const [showLeaveRoom, setShowLeaveRoom] = useState(false)
  const [deletingRoom, setDeletingRoom] = useState(false)
  const [leavingRoom, setLeavingRoom] = useState(false)
  const [removingIdentifier, setRemovingIdentifier] = useState<string | undefined>()

  const fetchParticipants = useCallback(async () => {
    setParticipantsLoading(true)
    try {
      const res = await communicateClient.listParticipants(room.id, { limit: 100 })
      setParticipants(res.items)
    } catch {
      // Fail silently — participants list is best-effort in settings
    } finally {
      setParticipantsLoading(false)
    }
  }, [room.id])

  useEffect(() => {
    setTopic(room.topic ?? '')
    setDescription(room.description ?? '')
    void fetchParticipants()
  }, [room.id, room.topic, room.description, fetchParticipants])

  // -------------------------------------------------------------------------
  // Handlers
  // -------------------------------------------------------------------------

  async function handleSaveInfo() {
    setSaving(true)
    setSaveError(undefined)
    try {
      const updated = await communicateClient.updateRoom(room.id, {
        topic: topic.trim() || undefined,
        description: description.trim() || undefined,
      })
      onRoomUpdated(updated)
    } catch (err) {
      setSaveError(mapApiError(err))
    } finally {
      setSaving(false)
    }
  }

  async function handleAddParticipant() {
    if (!addIdentifier.trim() || !addDisplayName.trim()) {
      setAddError('Identifier and display name are required')
      return
    }
    setAddError(undefined)
    setAdding(true)
    try {
      const p = await communicateClient.addParticipant(room.id, {
        identifier: addIdentifier.trim(),
        kind: addKind,
        display_name: addDisplayName.trim(),
        role: addRole,
      })
      setParticipants((prev) => [...prev, p])
      setAddIdentifier('')
      setAddDisplayName('')
    } catch (err) {
      setAddError(mapApiError(err))
    } finally {
      setAdding(false)
    }
  }

  async function handleRemoveParticipant(identifier: string) {
    setRemovingIdentifier(identifier)
    try {
      await communicateClient.removeParticipant(room.id, identifier)
      setParticipants((prev) => prev.filter((p) => p.identifier !== identifier))
    } catch {
      // Fail silently
    } finally {
      setRemovingIdentifier(undefined)
    }
  }

  async function handleDeleteRoom() {
    setDeletingRoom(true)
    try {
      await communicateClient.deleteRoom(room.id)
      onRoomDeleted()
    } catch {
      // Fail silently, close anyway
      onRoomDeleted()
    } finally {
      setDeletingRoom(false)
      setShowDeleteRoom(false)
    }
  }

  async function handleLeaveRoom() {
    setLeavingRoom(true)
    try {
      await communicateClient.removeParticipant(room.id, localIdentifier)
      onLeft()
    } catch {
      onLeft()
    } finally {
      setLeavingRoom(false)
      setShowLeaveRoom(false)
    }
  }

  const isLocalParticipant = participants.some((p) => p.identifier === localIdentifier)

  // -------------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------------

  return (
    <div className="flex h-full flex-col overflow-y-auto">
      {/* Header */}
      <div className="flex shrink-0 items-center justify-between border-b border-gray-700 px-4 py-3">
        <h3 className="text-sm font-semibold text-white">Room Settings</h3>
        <button
          type="button"
          onClick={onClose}
          aria-label="Close settings"
          className="rounded p-1 text-gray-400 hover:text-gray-200 transition-colors"
        >
          <X size={16} />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto space-y-5 px-4 py-4">
        {/* Room info */}
        <section aria-labelledby="room-info-heading">
          <h4 id="room-info-heading" className="mb-2 text-xs font-semibold uppercase tracking-wider text-gray-500">
            Info
          </h4>
          <dl className="space-y-1 text-xs">
            <div className="flex gap-2">
              <dt className="w-20 shrink-0 text-gray-500">Type</dt>
              <dd className="text-gray-300 capitalize">{room.room_type}</dd>
            </div>
            <div className="flex gap-2">
              <dt className="w-20 shrink-0 text-gray-500">Created by</dt>
              <dd className="text-gray-300 truncate">{room.created_by}</dd>
            </div>
            <div className="flex gap-2">
              <dt className="w-20 shrink-0 text-gray-500">Created</dt>
              <dd className="text-gray-300">
                {new Date(room.created_at).toLocaleDateString()}
              </dd>
            </div>
          </dl>
        </section>

        {/* Edit topic / description */}
        <section aria-labelledby="edit-room-heading">
          <h4 id="edit-room-heading" className="mb-2 text-xs font-semibold uppercase tracking-wider text-gray-500">
            Edit
          </h4>
          <div className="space-y-3">
            <div>
              <label className="mb-1 block text-xs font-medium text-gray-400">Topic</label>
              <input
                type="text"
                value={topic}
                onChange={(e) => setTopic(e.target.value)}
                placeholder="Room topic…"
                className="w-full rounded-md border border-gray-600 bg-gray-900 px-3 py-1.5 text-xs text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-primary-500"
              />
            </div>
            <div>
              <label className="mb-1 block text-xs font-medium text-gray-400">Description</label>
              <textarea
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="Room description…"
                rows={2}
                className="w-full resize-none rounded-md border border-gray-600 bg-gray-900 px-3 py-1.5 text-xs text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-primary-500"
              />
            </div>
            {saveError && <p className="text-xs text-red-400">{saveError}</p>}
            <button
              type="button"
              onClick={() => void handleSaveInfo()}
              disabled={saving}
              className="flex items-center gap-1.5 rounded-md bg-primary-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-primary-700 transition-colors disabled:opacity-50"
            >
              <Save size={12} />
              {saving ? 'Saving…' : 'Save changes'}
            </button>
          </div>
        </section>

        {/* Participants */}
        <section aria-labelledby="participants-heading">
          <h4 id="participants-heading" className="mb-2 text-xs font-semibold uppercase tracking-wider text-gray-500">
            Participants {!participantsLoading && `— ${participants.length}`}
          </h4>

          {/* Existing list */}
          <ul className="mb-3 space-y-1">
            {participants.map((p) => (
              <li
                key={p.id}
                className="flex items-center justify-between gap-2 rounded-md bg-gray-700/50 px-2 py-1.5"
              >
                <div className="min-w-0">
                  <p className="truncate text-xs font-medium text-gray-300">{p.display_name}</p>
                  <p className="truncate text-[10px] text-gray-500">{p.identifier}</p>
                </div>
                {p.identifier !== localIdentifier && (
                  <button
                    type="button"
                    onClick={() => void handleRemoveParticipant(p.identifier)}
                    disabled={removingIdentifier === p.identifier}
                    aria-label={`Remove ${p.display_name}`}
                    className="shrink-0 rounded p-1 text-gray-500 hover:text-red-400 transition-colors disabled:opacity-50"
                  >
                    <UserMinus size={12} />
                  </button>
                )}
              </li>
            ))}
          </ul>

          {/* Add participant form */}
          <div className="space-y-2">
            <div className="grid grid-cols-2 gap-2">
              <input
                type="text"
                value={addIdentifier}
                onChange={(e) => setAddIdentifier(e.target.value)}
                placeholder="identifier"
                className={miniField()}
              />
              <input
                type="text"
                value={addDisplayName}
                onChange={(e) => setAddDisplayName(e.target.value)}
                placeholder="Display name"
                className={miniField()}
              />
            </div>
            <div className="grid grid-cols-2 gap-2">
              <select
                value={addKind}
                onChange={(e) => setAddKind(e.target.value as ParticipantKind)}
                className={miniField()}
              >
                <option value="human">Human</option>
                <option value="agent">Agent</option>
              </select>
              <select
                value={addRole}
                onChange={(e) => setAddRole(e.target.value as ParticipantRole)}
                className={miniField()}
              >
                <option value="member">Member</option>
                <option value="admin">Admin</option>
                <option value="observer">Observer</option>
              </select>
            </div>
            {addError && <p className="text-xs text-red-400">{addError}</p>}
            <button
              type="button"
              onClick={() => void handleAddParticipant()}
              disabled={adding}
              className="flex w-full items-center justify-center gap-1.5 rounded-md border border-dashed border-gray-600 px-3 py-1.5 text-xs font-medium text-gray-400 hover:border-primary-500 hover:text-primary-400 transition-colors disabled:opacity-50"
            >
              <UserPlus size={12} />
              {adding ? 'Adding…' : 'Add participant'}
            </button>
          </div>
        </section>

        {/* Danger zone */}
        <section aria-labelledby="danger-heading" className="space-y-2">
          <h4 id="danger-heading" className="mb-2 text-xs font-semibold uppercase tracking-wider text-gray-500">
            Danger zone
          </h4>

          {isLocalParticipant && (
            <button
              type="button"
              onClick={() => setShowLeaveRoom(true)}
              className="flex w-full items-center gap-2 rounded-md border border-yellow-700/50 px-3 py-2 text-xs font-medium text-yellow-400 hover:bg-yellow-900/20 transition-colors"
            >
              <UserMinus size={12} />
              Leave room
            </button>
          )}

          <button
            type="button"
            onClick={() => setShowDeleteRoom(true)}
            className="flex w-full items-center gap-2 rounded-md border border-red-700/50 px-3 py-2 text-xs font-medium text-red-400 hover:bg-red-900/20 transition-colors"
          >
            <Trash2 size={12} />
            Delete room
          </button>
        </section>
      </div>

      {/* Confirm: delete room */}
      <ConfirmDialog
        open={showDeleteRoom}
        title="Delete room?"
        description={`This will permanently delete "${room.name}" and all its messages. This action cannot be undone.`}
        confirmLabel="Delete room"
        variant="danger"
        loading={deletingRoom}
        onConfirm={() => void handleDeleteRoom()}
        onCancel={() => setShowDeleteRoom(false)}
      />

      {/* Confirm: leave room */}
      <ConfirmDialog
        open={showLeaveRoom}
        title="Leave room?"
        description={`You will be removed from "${room.name}". You can rejoin later.`}
        confirmLabel="Leave"
        variant="danger"
        loading={leavingRoom}
        onConfirm={() => void handleLeaveRoom()}
        onCancel={() => setShowLeaveRoom(false)}
      />
    </div>
  )
}

function miniField(): string {
  return 'w-full rounded border border-gray-600 bg-gray-900 px-2 py-1 text-xs text-white placeholder-gray-500 focus:outline-none focus:ring-1 focus:ring-primary-500'
}
