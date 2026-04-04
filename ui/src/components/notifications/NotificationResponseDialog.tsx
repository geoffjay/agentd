/**
 * NotificationResponseDialog — modal for responding to actionable notifications.
 *
 * Shows full notification title, message, source details and a text area
 * for the user's response. On submit calls PUT /notifications/{id} with
 * status "Responded" and the response text.
 */

import { X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { Notification } from "@/types/notify";

const SOURCE_LABELS: Record<string, string> = {
	system: "System",
	ask_service: "Ask Service",
	agent_hook: "Agent Hook",
	monitor_service: "Monitor Service",
};

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface NotificationResponseDialogProps {
	notification: Notification | null;
	/** Whether the submit action is in progress */
	busy?: boolean;
	onSubmit: (id: string, response: string) => Promise<void>;
	onClose: () => void;
}

export function NotificationResponseDialog({
	notification,
	busy = false,
	onSubmit,
	onClose,
}: NotificationResponseDialogProps) {
	const [responseText, setResponseText] = useState("");
	const [error, setError] = useState<string | undefined>();
	const textareaRef = useRef<HTMLTextAreaElement>(null);
	const dialogRef = useRef<HTMLDivElement>(null);

	// Reset when notification changes
	useEffect(() => {
		setResponseText("");
		setError(undefined);
	}, [notification?.id]);

	// Focus textarea when dialog opens
	useEffect(() => {
		if (notification) {
			setTimeout(() => textareaRef.current?.focus(), 50);
		}
	}, [notification]);

	// Close on Escape
	useEffect(() => {
		if (!notification) return;
		const handler = (e: KeyboardEvent) => {
			if (e.key === "Escape") onClose();
		};
		document.addEventListener("keydown", handler);
		return () => document.removeEventListener("keydown", handler);
	}, [notification, onClose]);

	if (!notification) return null;

	const handleSubmit = async (e: React.FormEvent) => {
		e.preventDefault();
		const trimmed = responseText.trim();
		if (!trimmed) {
			setError("Response cannot be empty.");
			return;
		}
		setError(undefined);
		try {
			await onSubmit(notification.id, trimmed);
			onClose();
		} catch (err) {
			setError(
				err instanceof Error ? err.message : "Failed to submit response.",
			);
		}
	};

	return (
		/* Backdrop */
		<div
			className="fixed inset-0 z-50 flex items-center justify-center bg-th-overlay p-4"
			aria-modal="true"
			role="dialog"
			aria-labelledby="response-dialog-title"
			onClick={(e) => {
				if (e.target === e.currentTarget) onClose();
			}}
		>
			<div
				ref={dialogRef}
				className="w-full max-w-lg rounded-xl border border-th-border bg-th-surface shadow-2xl"
			>
				{/* Header */}
				<div className="flex items-start justify-between gap-4 border-b border-th-border px-6 py-4">
					<div className="min-w-0">
						<h2
							id="response-dialog-title"
							className="text-base font-semibold text-th-text truncate"
						>
							{notification.title}
						</h2>
						<p className="mt-0.5 text-xs text-th-text-muted">
							{SOURCE_LABELS[notification.source.type] ??
								notification.source.type}
							{" · "}
							{notification.priority} priority
						</p>
					</div>
					<button
						type="button"
						aria-label="Close dialog"
						onClick={onClose}
						className="shrink-0 rounded-md p-1 text-th-text-muted hover:bg-th-surface-hover hover:text-th-text transition-colors"
					>
						<X size={18} />
					</button>
				</div>

				{/* Body */}
				<div className="px-6 py-4 space-y-4">
					{/* Full message */}
					<div className="rounded-md bg-th-surface-sunken p-4 text-sm text-th-text-secondary whitespace-pre-wrap max-h-40 overflow-y-auto">
						{notification.message}
					</div>

					{/* Response form */}
					<form onSubmit={handleSubmit} className="space-y-3">
						<label
							htmlFor="response-input"
							className="block text-sm font-medium text-th-text-secondary"
						>
							Your response
						</label>
						<textarea
							id="response-input"
							ref={textareaRef}
							value={responseText}
							onChange={(e) => setResponseText(e.target.value)}
							rows={4}
							placeholder="Type your response here…"
							disabled={busy}
							className="w-full rounded-md border border-th-border-input bg-th-input px-3 py-2 text-sm text-th-text placeholder:text-th-text-faint focus:border-th-focus-ring focus:outline-none focus:ring-1 focus:ring-th-focus-ring disabled:opacity-50 resize-none"
						/>

						{error && (
							<p role="alert" className="text-xs text-th-status-error-text">
								{error}
							</p>
						)}

						{/* Actions */}
						<div className="flex justify-end gap-2">
							<button
								type="button"
								onClick={onClose}
								disabled={busy}
								className="rounded-md px-4 py-2 text-sm font-medium text-th-text-muted hover:text-th-text hover:bg-th-surface-hover transition-colors disabled:opacity-50"
							>
								Cancel
							</button>
							<button
								type="submit"
								disabled={busy || !responseText.trim()}
								className="rounded-md bg-th-accent px-4 py-2 text-sm font-medium text-th-accent-text hover:bg-th-accent-hover transition-colors disabled:opacity-50"
							>
								{busy ? "Submitting…" : "Submit Response"}
							</button>
						</div>
					</form>
				</div>
			</div>
		</div>
	);
}

export default NotificationResponseDialog;
