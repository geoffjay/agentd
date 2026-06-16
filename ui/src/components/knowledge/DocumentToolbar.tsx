/**
 * DocumentToolbar — actions above the editor: create, rename, delete.
 */

import { FilePlus, Save, Trash2 } from "lucide-react";
import { useState } from "react";
import type { Document } from "@/types/knowledge";

interface DocumentToolbarProps {
	document: Document | null;
	saving: boolean;
	projectId: string | null;
	onCreateClick: () => void;
	onDeleteClick: () => void;
}

export function DocumentToolbar({
	document,
	saving,
	projectId,
	onCreateClick,
	onDeleteClick,
}: DocumentToolbarProps) {
	return (
		<div className="flex items-center gap-2 border-b border-th-border bg-th-surface px-4 py-2">
			{/* Document title / path */}
			<div className="flex-1 min-w-0">
				{document ? (
					<div className="flex items-baseline gap-2">
						<span className="text-sm font-semibold text-th-text truncate">
							{document.title}
						</span>
						<span className="text-xs text-th-text-muted truncate">
							{document.rel_path}
						</span>
					</div>
				) : (
					<span className="text-sm text-th-text-muted">
						{projectId ? "Select a document" : "Select a project"}
					</span>
				)}
			</div>

			{/* Autosave indicator */}
			{saving && (
				<span className="flex items-center gap-1 text-xs text-th-text-muted">
					<Save size={12} className="animate-pulse" />
					Saving…
				</span>
			)}

			{/* Actions */}
			{projectId && (
				<button
					type="button"
					onClick={onCreateClick}
					title="New document"
					className="flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium text-th-text hover:bg-th-surface-secondary"
				>
					<FilePlus size={14} />
					New
				</button>
			)}
			{document && (
				<button
					type="button"
					onClick={onDeleteClick}
					title="Delete document"
					className="flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium text-th-status-error-text hover:bg-th-status-error-bg"
				>
					<Trash2 size={14} />
					Delete
				</button>
			)}
		</div>
	);
}

// ---------------------------------------------------------------------------
// Create document dialog
// ---------------------------------------------------------------------------

interface CreateDocumentDialogProps {
	onConfirm: (relPath: string, title: string) => void;
	onCancel: () => void;
}

export function CreateDocumentDialog({
	onConfirm,
	onCancel,
}: CreateDocumentDialogProps) {
	const [relPath, setRelPath] = useState("");
	const [title, setTitle] = useState("");

	function handleSubmit(e: React.FormEvent) {
		e.preventDefault();
		const path = relPath.trim();
		if (!path) return;
		const finalPath = path.endsWith(".md") ? path : `${path}.md`;
		onConfirm(finalPath, title.trim());
	}

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center bg-th-overlay">
			<div className="w-full max-w-md rounded-lg border border-th-border bg-th-surface p-6 shadow-xl">
				<h2 className="mb-4 text-base font-semibold text-th-text">
					New Document
				</h2>
				<form onSubmit={handleSubmit} className="space-y-3">
					<div>
						<label
							htmlFor="rel-path"
							className="mb-1 block text-xs font-medium text-th-text-muted"
						>
							Path (relative, e.g. docs/api.md)
						</label>
						<input
							id="rel-path"
							type="text"
							value={relPath}
							onChange={(e) => setRelPath(e.target.value)}
							placeholder="readme.md"
							required
							className="w-full rounded-md border border-th-border bg-th-background px-3 py-2 text-sm text-th-text focus:outline-none focus:ring-2 focus:ring-th-border-focus"
						/>
					</div>
					<div>
						<label
							htmlFor="doc-title"
							className="mb-1 block text-xs font-medium text-th-text-muted"
						>
							Title (optional)
						</label>
						<input
							id="doc-title"
							type="text"
							value={title}
							onChange={(e) => setTitle(e.target.value)}
							placeholder="Leave blank to use filename"
							className="w-full rounded-md border border-th-border bg-th-background px-3 py-2 text-sm text-th-text focus:outline-none focus:ring-2 focus:ring-th-border-focus"
						/>
					</div>
					<div className="flex justify-end gap-2 pt-2">
						<button
							type="button"
							onClick={onCancel}
							className="rounded-md px-3 py-1.5 text-sm text-th-text hover:bg-th-surface-secondary"
						>
							Cancel
						</button>
						<button
							type="submit"
							className="rounded-md bg-th-button-primary px-3 py-1.5 text-sm font-medium text-th-button-primary-text hover:bg-th-button-primary-hover"
						>
							Create
						</button>
					</div>
				</form>
			</div>
		</div>
	);
}
