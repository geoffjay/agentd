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

import { Save, Trash2, UserMinus, UserPlus, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { mapApiError, useToast } from "@/hooks/useToast";
import { communicateClient } from "@/services/communicate";
import type {
	Participant,
	ParticipantKind,
	ParticipantRole,
	Room,
} from "@/types/communicate";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface RoomSettingsPanelProps {
	room: Room;
	localIdentifier: string;
	onClose: () => void;
	onRoomDeleted: () => void;
	onLeft: () => void;
	onRoomUpdated: (updated: Room) => void;
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
	const toast = useToast();
	const toastRef = useRef(toast);
	toastRef.current = toast;

	// Editable fields
	const [topic, setTopic] = useState(room.topic ?? "");
	const [description, setDescription] = useState(room.description ?? "");
	const [saving, setSaving] = useState(false);
	const [saveError, setSaveError] = useState<string | undefined>();

	// Participants
	const [participants, setParticipants] = useState<Participant[]>([]);
	const [participantsLoading, setParticipantsLoading] = useState(false);

	// Add participant form
	const [addIdentifier, setAddIdentifier] = useState("");
	const [addDisplayName, setAddDisplayName] = useState("");
	const [addKind, setAddKind] = useState<ParticipantKind>("human");
	const [addRole, setAddRole] = useState<ParticipantRole>("member");
	const [addError, setAddError] = useState<string | undefined>();
	const [adding, setAdding] = useState(false);

	// Confirm dialogs
	const [showDeleteRoom, setShowDeleteRoom] = useState(false);
	const [showLeaveRoom, setShowLeaveRoom] = useState(false);
	const [deletingRoom, setDeletingRoom] = useState(false);
	const [leavingRoom, setLeavingRoom] = useState(false);
	const [removingIdentifier, setRemovingIdentifier] = useState<
		string | undefined
	>();

	const fetchParticipants = useCallback(async () => {
		setParticipantsLoading(true);
		try {
			const res = await communicateClient.listParticipants(room.id, {
				limit: 100,
			});
			setParticipants(res.items);
		} catch {
			// Fail silently — participants list is best-effort in settings
		} finally {
			setParticipantsLoading(false);
		}
	}, [room.id]);

	// Fetch participants only when the room changes (not on every topic/description save)
	useEffect(() => {
		void fetchParticipants();
	}, [fetchParticipants]);

	// Sync editable fields when the room prop updates (e.g. after a save)
	useEffect(() => {
		setTopic(room.topic ?? "");
		setDescription(room.description ?? "");
	}, [room.topic, room.description]);

	// -------------------------------------------------------------------------
	// Handlers
	// -------------------------------------------------------------------------

	async function handleSaveInfo() {
		setSaving(true);
		setSaveError(undefined);
		try {
			const updated = await communicateClient.updateRoom(room.id, {
				topic: topic.trim() || undefined,
				description: description.trim() || undefined,
			});
			onRoomUpdated(updated);
		} catch (err) {
			setSaveError(mapApiError(err));
		} finally {
			setSaving(false);
		}
	}

	async function handleAddParticipant() {
		if (!addIdentifier.trim() || !addDisplayName.trim()) {
			setAddError("Identifier and display name are required");
			return;
		}
		setAddError(undefined);
		setAdding(true);
		try {
			const p = await communicateClient.addParticipant(room.id, {
				identifier: addIdentifier.trim(),
				kind: addKind,
				display_name: addDisplayName.trim(),
				role: addRole,
			});
			setParticipants((prev) => [...prev, p]);
			setAddIdentifier("");
			setAddDisplayName("");
		} catch (err) {
			setAddError(mapApiError(err));
		} finally {
			setAdding(false);
		}
	}

	async function handleRemoveParticipant(identifier: string) {
		setRemovingIdentifier(identifier);
		try {
			await communicateClient.removeParticipant(room.id, identifier);
			setParticipants((prev) =>
				prev.filter((p) => p.identifier !== identifier),
			);
		} catch (err) {
			toastRef.current.error("Failed to remove participant", {
				message: mapApiError(err),
			});
		} finally {
			setRemovingIdentifier(undefined);
		}
	}

	async function handleDeleteRoom() {
		setDeletingRoom(true);
		try {
			await communicateClient.deleteRoom(room.id);
			onRoomDeleted();
		} catch (err) {
			// Do NOT invoke onRoomDeleted — the room still exists on the server.
			toastRef.current.error("Failed to delete room", {
				message: mapApiError(err),
			});
			setShowDeleteRoom(false);
		} finally {
			setDeletingRoom(false);
		}
	}

	async function handleLeaveRoom() {
		setLeavingRoom(true);
		try {
			await communicateClient.removeParticipant(room.id, localIdentifier);
			onLeft();
		} catch (err) {
			// Do NOT invoke onLeft — the participant is still in the room on the server.
			toastRef.current.error("Failed to leave room", {
				message: mapApiError(err),
			});
			setShowLeaveRoom(false);
		} finally {
			setLeavingRoom(false);
		}
	}

	const isLocalParticipant = participants.some(
		(p) => p.identifier === localIdentifier,
	);

	// -------------------------------------------------------------------------
	// Render
	// -------------------------------------------------------------------------

	return (
		<div className="flex h-full flex-col overflow-y-auto">
			{/* Header */}
			<div className="flex shrink-0 items-center justify-between border-b border-th-border-nav px-4 py-3">
				<h3 className="text-sm font-semibold text-th-text">Room Settings</h3>
				<button
					type="button"
					onClick={onClose}
					aria-label="Close settings"
					className="rounded p-1 text-th-text-muted hover:text-th-text transition-colors"
				>
					<X size={16} />
				</button>
			</div>

			<div className="flex-1 overflow-y-auto space-y-5 px-4 py-4">
				{/* Room info */}
				<section aria-labelledby="room-info-heading">
					<h4
						id="room-info-heading"
						className="mb-2 text-xs font-semibold uppercase tracking-wider text-th-text-muted"
					>
						Info
					</h4>
					<dl className="space-y-1 text-xs">
						<div className="flex gap-2">
							<dt className="w-20 shrink-0 text-th-text-muted">Type</dt>
							<dd className="text-th-text-secondary capitalize">
								{room.room_type}
							</dd>
						</div>
						<div className="flex gap-2">
							<dt className="w-20 shrink-0 text-th-text-muted">Created by</dt>
							<dd className="text-th-text-secondary truncate">
								{room.created_by}
							</dd>
						</div>
						<div className="flex gap-2">
							<dt className="w-20 shrink-0 text-th-text-muted">Created</dt>
							<dd className="text-th-text-secondary">
								{new Date(room.created_at).toLocaleDateString()}
							</dd>
						</div>
					</dl>
				</section>

				{/* Edit topic / description */}
				<section aria-labelledby="edit-room-heading">
					<h4
						id="edit-room-heading"
						className="mb-2 text-xs font-semibold uppercase tracking-wider text-th-text-muted"
					>
						Edit
					</h4>
					<div className="space-y-3">
						<div>
							<label className="mb-1 block text-xs font-medium text-th-text-muted">
								Topic
							</label>
							<input
								type="text"
								value={topic}
								onChange={(e) => setTopic(e.target.value)}
								placeholder="Room topic…"
								className="w-full rounded-md border border-th-border-input bg-th-input px-3 py-1.5 text-xs text-th-text placeholder-th-text-faint focus:outline-none focus:ring-2 focus:ring-th-focus-ring"
							/>
						</div>
						<div>
							<label className="mb-1 block text-xs font-medium text-th-text-muted">
								Description
							</label>
							<textarea
								value={description}
								onChange={(e) => setDescription(e.target.value)}
								placeholder="Room description…"
								rows={2}
								className="w-full resize-none rounded-md border border-th-border-input bg-th-input px-3 py-1.5 text-xs text-th-text placeholder-th-text-faint focus:outline-none focus:ring-2 focus:ring-th-focus-ring"
							/>
						</div>
						{saveError && (
							<p className="text-xs text-th-status-error-text">{saveError}</p>
						)}
						<button
							type="button"
							onClick={() => void handleSaveInfo()}
							disabled={saving}
							className="flex items-center gap-1.5 rounded-md bg-th-accent px-3 py-1.5 text-xs font-medium text-th-accent-text hover:bg-th-accent-hover transition-colors disabled:opacity-50"
						>
							<Save size={12} />
							{saving ? "Saving…" : "Save changes"}
						</button>
					</div>
				</section>

				{/* Participants */}
				<section aria-labelledby="participants-heading">
					<h4
						id="participants-heading"
						className="mb-2 text-xs font-semibold uppercase tracking-wider text-th-text-muted"
					>
						Participants {!participantsLoading && `— ${participants.length}`}
					</h4>

					{/* Existing list */}
					<ul className="mb-3 space-y-1">
						{participants.map((p) => (
							<li
								key={p.id}
								className="flex items-center justify-between gap-2 rounded-md bg-th-surface-raised px-2 py-1.5"
							>
								<div className="min-w-0">
									<p className="truncate text-xs font-medium text-th-text-secondary">
										{p.display_name}
									</p>
									<p className="truncate text-[10px] text-th-text-muted">
										{p.identifier}
									</p>
								</div>
								{p.identifier !== localIdentifier && (
									<button
										type="button"
										onClick={() => void handleRemoveParticipant(p.identifier)}
										disabled={removingIdentifier === p.identifier}
										aria-label={`Remove ${p.display_name}`}
										className="shrink-0 rounded p-1 text-th-text-muted hover:text-th-status-error-text transition-colors disabled:opacity-50"
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
						{addError && (
							<p className="text-xs text-th-status-error-text">{addError}</p>
						)}
						<button
							type="button"
							onClick={() => void handleAddParticipant()}
							disabled={adding}
							className="flex w-full items-center justify-center gap-1.5 rounded-md border border-dashed border-th-border px-3 py-1.5 text-xs font-medium text-th-text-muted hover:border-th-accent hover:text-th-text-link transition-colors disabled:opacity-50"
						>
							<UserPlus size={12} />
							{adding ? "Adding…" : "Add participant"}
						</button>
					</div>
				</section>

				{/* Danger zone */}
				<section aria-labelledby="danger-heading" className="space-y-2">
					<h4
						id="danger-heading"
						className="mb-2 text-xs font-semibold uppercase tracking-wider text-th-text-muted"
					>
						Danger zone
					</h4>

					{isLocalParticipant && (
						<button
							type="button"
							onClick={() => setShowLeaveRoom(true)}
							className="flex w-full items-center gap-2 rounded-md border border-th-status-warning-border px-3 py-2 text-xs font-medium text-th-status-warning-text hover:opacity-80 transition-colors"
						>
							<UserMinus size={12} />
							Leave room
						</button>
					)}

					<button
						type="button"
						onClick={() => setShowDeleteRoom(true)}
						className="flex w-full items-center gap-2 rounded-md border border-th-status-error-border px-3 py-2 text-xs font-medium text-th-status-error-text hover:opacity-80 transition-colors"
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
	);
}

function miniField(): string {
	return "w-full rounded border border-th-border-input bg-th-input px-2 py-1 text-xs text-th-text placeholder-th-text-faint focus:outline-none focus:ring-1 focus:ring-th-focus-ring";
}
