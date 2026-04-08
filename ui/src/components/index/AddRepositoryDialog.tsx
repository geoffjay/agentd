/**
 * AddRepositoryDialog — modal form for registering a new repository with the
 * index service.
 *
 * Fields:
 * - Name (required) — display name for the repository
 * - Path (required) — absolute filesystem path to the repository root
 *
 * Follows the CreateMemoryDialog pattern with:
 * - Focus trap and ESC to close
 * - Client-side validation with inline error messages
 * - Saving state with disabled buttons
 */

import { X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { FocusTrap } from "@/components/common/FocusTrap";
import type { AddRepoRequest } from "@/types/codeindex";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface AddRepositoryDialogProps {
	open: boolean;
	onSave: (request: AddRepoRequest) => Promise<boolean>;
	onClose: () => void;
}

interface FormErrors {
	name?: string;
	path?: string;
}

// ---------------------------------------------------------------------------
// Style helpers
// ---------------------------------------------------------------------------

function fieldClass(error?: string): string {
	return [
		"w-full rounded-md border px-3 py-2 text-sm",
		"bg-th-input text-th-text",
		"focus:outline-none focus:ring-2 focus:ring-th-focus-ring",
		"disabled:cursor-not-allowed disabled:opacity-50",
		error ? "border-th-status-error-border" : "border-th-border-input",
	].join(" ");
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function AddRepositoryDialog({
	open,
	onSave,
	onClose,
}: AddRepositoryDialogProps) {
	const nameRef = useRef<HTMLInputElement>(null);

	const [name, setName] = useState("");
	const [path, setPath] = useState("");
	const [errors, setErrors] = useState<FormErrors>({});
	const [saving, setSaving] = useState(false);
	const [saveError, setSaveError] = useState<string | undefined>();

	// Reset form on open
	useEffect(() => {
		if (!open) return;
		setName("");
		setPath("");
		setErrors({});
		setSaveError(undefined);
		setSaving(false);
		setTimeout(() => nameRef.current?.focus(), 50);
	}, [open]);

	if (!open) return null;

	function validate(): FormErrors {
		const e: FormErrors = {};
		if (!name.trim()) e.name = "Name is required";
		if (!path.trim()) e.path = "Path is required";
		return e;
	}

	async function handleSave() {
		const e = validate();
		if (Object.keys(e).length > 0) {
			setErrors(e);
			return;
		}

		setSaving(true);
		setSaveError(undefined);
		try {
			const ok = await onSave({ name: name.trim(), path: path.trim() });
			if (ok) {
				onClose();
			} else {
				setSaveError("Failed to add repository — check the path and try again.");
			}
		} catch (err) {
			setSaveError(
				err instanceof Error ? err.message : "Failed to add repository",
			);
		} finally {
			setSaving(false);
		}
	}

	function handleKeyDown(e: React.KeyboardEvent) {
		if (e.key === "Enter" && !saving) void handleSave();
	}

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center p-4">
			{/* Backdrop */}
			<div
				className="absolute inset-0 bg-th-overlay"
				onClick={onClose}
				aria-hidden="true"
			/>

			<FocusTrap active onEscape={onClose}>
				<div
					role="dialog"
					aria-modal="true"
					aria-labelledby="add-repo-title"
					className="relative z-10 w-full max-w-lg rounded-xl bg-th-surface shadow-xl"
					onKeyDown={handleKeyDown}
				>
					{/* Header */}
					<div className="flex items-center justify-between border-b border-th-border bg-th-surface px-6 py-4">
						<h2
							id="add-repo-title"
							className="text-lg font-semibold text-th-text"
						>
							Add Repository
						</h2>
						<button
							type="button"
							onClick={onClose}
							className="rounded p-1 text-th-text-muted hover:text-th-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-th-focus-ring"
							aria-label="Close dialog"
						>
							<X size={18} />
						</button>
					</div>

					{/* Body */}
					<div className="px-6 py-5 space-y-4">
						{saveError && (
							<p
								role="alert"
								className="rounded-md bg-th-status-error-bg px-3 py-2 text-sm text-th-status-error-text"
							>
								{saveError}
							</p>
						)}

						{/* Name */}
						<div>
							<label
								htmlFor="repo-name"
								className="block text-sm font-medium text-th-text-secondary mb-1"
							>
								Name <span className="text-th-status-error-text">*</span>
							</label>
							<input
								id="repo-name"
								ref={nameRef}
								type="text"
								value={name}
								onChange={(e) => setName(e.target.value)}
								placeholder="e.g. agentd"
								className={fieldClass(errors.name)}
								disabled={saving}
							/>
							{errors.name && (
								<p className="mt-1 text-xs text-th-status-error-text">
									{errors.name}
								</p>
							)}
						</div>

						{/* Path */}
						<div>
							<label
								htmlFor="repo-path"
								className="block text-sm font-medium text-th-text-secondary mb-1"
							>
								Path <span className="text-th-status-error-text">*</span>
							</label>
							<input
								id="repo-path"
								type="text"
								value={path}
								onChange={(e) => setPath(e.target.value)}
								placeholder="e.g. /home/user/projects/agentd"
								className={fieldClass(errors.path)}
								disabled={saving}
							/>
							{errors.path && (
								<p className="mt-1 text-xs text-th-status-error-text">
									{errors.path}
								</p>
							)}
							<p className="mt-1 text-xs text-th-text-muted">
								Absolute path on the server where the index service can read the
								source files.
							</p>
						</div>
					</div>

					{/* Footer */}
					<div className="flex items-center justify-end gap-3 border-t border-th-border bg-th-surface px-6 py-4">
						<button
							type="button"
							onClick={onClose}
							disabled={saving}
							className="rounded-md border border-th-border-strong bg-th-surface px-4 py-2 text-sm font-medium text-th-text-secondary hover:bg-th-surface-hover transition-colors disabled:opacity-50"
						>
							Cancel
						</button>
						<button
							type="button"
							onClick={() => void handleSave()}
							disabled={saving}
							className="rounded-md bg-th-accent px-4 py-2 text-sm font-medium text-th-accent-text hover:bg-th-accent-hover transition-colors disabled:opacity-50"
						>
							{saving ? "Adding…" : "Add repository"}
						</button>
					</div>
				</div>
			</FocusTrap>
		</div>
	);
}

export default AddRepositoryDialog;
