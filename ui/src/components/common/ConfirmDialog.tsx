/**
 * ConfirmDialog — reusable confirmation modal.
 *
 * Renders a centered dialog with a title, description, and confirm/cancel buttons.
 * The confirm button can be styled as 'danger' (red) for destructive actions.
 */

import { X } from "lucide-react";
import { useEffect, useRef } from "react";

export interface ConfirmDialogProps {
	/** Whether the dialog is currently visible */
	open: boolean;
	/** Dialog title */
	title: string;
	/** Descriptive text body */
	description?: string;
	/** Label for the confirm button (default: "Confirm") */
	confirmLabel?: string;
	/** Label for the cancel button (default: "Cancel") */
	cancelLabel?: string;
	/** 'danger' = red confirm button; 'primary' = blue (default) */
	variant?: "danger" | "primary";
	/** Whether the confirm action is in progress */
	loading?: boolean;
	onConfirm: () => void;
	onCancel: () => void;
}

export function ConfirmDialog({
	open,
	title,
	description,
	confirmLabel = "Confirm",
	cancelLabel = "Cancel",
	variant = "primary",
	loading = false,
	onConfirm,
	onCancel,
}: ConfirmDialogProps) {
	const cancelRef = useRef<HTMLButtonElement>(null);

	// Focus the cancel button when dialog opens (safe default)
	useEffect(() => {
		if (open) {
			cancelRef.current?.focus();
		}
	}, [open]);

	// Close on Escape
	useEffect(() => {
		if (!open) return;
		function handleKey(e: KeyboardEvent) {
			if (e.key === "Escape") onCancel();
		}
		document.addEventListener("keydown", handleKey);
		return () => document.removeEventListener("keydown", handleKey);
	}, [open, onCancel]);

	if (!open) return null;

	const confirmClasses =
		variant === "danger"
			? "bg-th-status-error-dot text-th-accent-text hover:opacity-90 focus:ring-th-focus-ring"
			: "bg-th-accent text-th-accent-text hover:bg-th-accent-hover focus:ring-th-focus-ring";

	return (
		/* Backdrop */
		<div
			aria-hidden={!open}
			className="fixed inset-0 z-50 flex items-center justify-center p-4"
		>
			{/* Overlay */}
			<div
				className="absolute inset-0 bg-th-overlay"
				aria-hidden="true"
				onClick={onCancel}
			/>

			{/* Dialog panel */}
			<div
				role="alertdialog"
				aria-modal="true"
				aria-labelledby="confirm-dialog-title"
				aria-describedby={description ? "confirm-dialog-desc" : undefined}
				className="relative rounded-lg bg-th-surface p-6 shadow-xl"
			>
				{/* Close button */}
				<button
					type="button"
					aria-label="Close dialog"
					onClick={onCancel}
					className="absolute right-4 top-4 rounded-md p-1 text-th-text-muted hover:bg-th-surface-hover hover:text-th-text"
				>
					<X size={16} />
				</button>

				{/* Title */}
				<h2
					id="confirm-dialog-title"
					className="text-base font-semibold text-th-text"
				>
					{title}
				</h2>

				{/* Description */}
				{description && (
					<p
						id="confirm-dialog-desc"
						className="mt-2 text-sm text-th-text-muted"
					>
						{description}
					</p>
				)}

				{/* Actions */}
				<div className="mt-5 flex justify-end gap-3">
					<button
						ref={cancelRef}
						type="button"
						onClick={onCancel}
						disabled={loading}
						className="rounded-md border border-th-border-strong bg-th-surface px-4 py-2 text-sm font-medium text-th-text-secondary hover:bg-th-surface-hover focus:outline-none focus:ring-2 focus:ring-th-focus-ring focus:ring-offset-2 disabled:opacity-50"
					>
						{cancelLabel}
					</button>
					<button
						type="button"
						onClick={onConfirm}
						disabled={loading}
						className={[
							"rounded-md px-4 py-2 text-sm font-medium focus:outline-none focus:ring-2 focus:ring-offset-2 disabled:opacity-50 transition-colors",
							confirmClasses,
						].join(" ")}
					>
						{loading ? "Processing…" : confirmLabel}
					</button>
				</div>
			</div>
		</div>
	);
}

export default ConfirmDialog;
