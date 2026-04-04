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

import { X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { FocusTrap } from "@/components/common/FocusTrap";
import { mapApiError } from "@/hooks/useToast";
import { communicateClient } from "@/services/communicate";
import type { Room, RoomType } from "@/types/communicate";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface CreateRoomDialogProps {
	open: boolean;
	/** The identifier of the person creating the room. */
	createdBy: string;
	onCreated: (room: Room) => void;
	onClose: () => void;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const ROOM_TYPE_OPTIONS: Array<{
	value: RoomType;
	label: string;
	description: string;
}> = [
	{ value: "group", label: "Group", description: "Open group conversation" },
	{
		value: "direct",
		label: "Direct",
		description: "Private 1-on-1 or small group",
	},
	{
		value: "broadcast",
		label: "Broadcast",
		description: "One-way announcements",
	},
];

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function CreateRoomDialog({
	open,
	createdBy,
	onCreated,
	onClose,
}: CreateRoomDialogProps) {
	const nameRef = useRef<HTMLInputElement>(null);

	const [name, setName] = useState("");
	const [roomType, setRoomType] = useState<RoomType>("group");
	const [topic, setTopic] = useState("");
	const [description, setDescription] = useState("");
	const [nameError, setNameError] = useState<string | undefined>();
	const [saveError, setSaveError] = useState<string | undefined>();
	const [saving, setSaving] = useState(false);

	// Reset form when dialog opens
	useEffect(() => {
		if (!open) return;
		setName("");
		setRoomType("group");
		setTopic("");
		setDescription("");
		setNameError(undefined);
		setSaveError(undefined);
		setSaving(false);
		setTimeout(() => nameRef.current?.focus(), 50);
	}, [open]);

	if (!open) return null;

	async function handleCreate() {
		if (!name.trim()) {
			setNameError("Room name is required");
			return;
		}
		setNameError(undefined);
		setSaveError(undefined);
		setSaving(true);
		try {
			const room = await communicateClient.createRoom({
				name: name.trim(),
				room_type: roomType,
				topic: topic.trim() || undefined,
				description: description.trim() || undefined,
				created_by: createdBy,
			});
			onCreated(room);
		} catch (err) {
			setSaveError(mapApiError(err));
		} finally {
			setSaving(false);
		}
	}

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center p-4">
			<div
				className="absolute inset-0 bg-th-overlay"
				onClick={onClose}
				aria-hidden="true"
			/>
			<FocusTrap active onEscape={onClose}>
				<div
					role="dialog"
					aria-modal="true"
					aria-labelledby="create-room-title"
					className="relative z-10 rounded-xl bg-th-surface shadow-xl border border-th-border"
				>
					{/* Header */}
					<div className="flex items-center justify-between border-b border-th-border px-6 py-4">
						<h2
							id="create-room-title"
							className="text-base font-semibold text-th-text"
						>
							Create Room
						</h2>
						<button
							type="button"
							onClick={onClose}
							aria-label="Close dialog"
							className="rounded p-1 text-th-text-muted hover:text-th-text transition-colors"
						>
							<X size={18} />
						</button>
					</div>

					{/* Body */}
					<div className="px-6 py-5 space-y-4">
						{saveError && (
							<p className="rounded-md bg-th-status-error-bg border border-th-status-error-border px-3 py-2 text-sm text-th-status-error-text">
								{saveError}
							</p>
						)}

						{/* Name */}
						<div>
							<label className="block text-sm font-medium text-th-text-secondary mb-1">
								Name <span className="text-th-status-error-text">*</span>
							</label>
							<input
								ref={nameRef}
								type="text"
								value={name}
								onChange={(e) => setName(e.target.value)}
								placeholder="e.g. general, ops-team"
								className={fieldClass(nameError)}
							/>
							{nameError && (
								<p className="mt-1 text-xs text-th-status-error-text">{nameError}</p>
							)}
						</div>

						{/* Room type */}
						<div>
							<label className="block text-sm font-medium text-th-text-secondary mb-1">
								Type
							</label>
							<div className="grid grid-cols-3 gap-2">
								{ROOM_TYPE_OPTIONS.map((opt) => (
									<button
										key={opt.value}
										type="button"
										onClick={() => setRoomType(opt.value)}
										className={[
											"flex flex-col items-start rounded-md border px-3 py-2 text-left text-xs transition-colors",
											roomType === opt.value
												? "border-th-focus-ring bg-th-accent/10 text-th-text-link"
												: "border-th-border-strong bg-th-surface-raised text-th-text-secondary hover:border-th-border-strong",
										].join(" ")}
									>
										<span className="font-medium">{opt.label}</span>
										<span className="text-[10px] text-th-text-muted mt-0.5">
											{opt.description}
										</span>
									</button>
								))}
							</div>
						</div>

						{/* Topic */}
						<div>
							<label className="block text-sm font-medium text-th-text-secondary mb-1">
								Topic{" "}
								<span className="text-th-text-muted font-normal">(optional)</span>
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
							<label className="block text-sm font-medium text-th-text-secondary mb-1">
								Description{" "}
								<span className="text-th-text-muted font-normal">(optional)</span>
							</label>
							<textarea
								value={description}
								onChange={(e) => setDescription(e.target.value)}
								placeholder="What is this room for?"
								rows={2}
								className={fieldClass() + " resize-none"}
							/>
						</div>
					</div>

					{/* Footer */}
					<div className="flex justify-end gap-3 border-t border-th-border px-6 py-4">
						<button
							type="button"
							onClick={onClose}
							disabled={saving}
							className="rounded-md border border-th-border-strong px-4 py-2 text-sm font-medium text-th-text-secondary hover:bg-th-surface-hover transition-colors disabled:opacity-50"
						>
							Cancel
						</button>
						<button
							type="button"
							onClick={() => void handleCreate()}
							disabled={saving}
							className="rounded-md bg-th-accent px-4 py-2 text-sm font-medium text-th-accent-text hover:bg-th-accent-hover transition-colors disabled:opacity-50"
						>
							{saving ? "Creating…" : "Create room"}
						</button>
					</div>
				</div>
			</FocusTrap>
		</div>
	);
}

function fieldClass(error?: string): string {
	return [
		"w-full rounded-md border px-3 py-2 text-sm bg-th-input text-th-text",
		"placeholder:text-th-text-faint focus:outline-none focus:ring-2 focus:ring-th-focus-ring",
		"disabled:opacity-50",
		error ? "border-th-status-error-border" : "border-th-border-input",
	].join(" ");
}
