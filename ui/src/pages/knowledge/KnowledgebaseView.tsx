/**
 * KnowledgebaseView — standard page header (title + project picker + actions)
 * over a bordered two-pane workspace: document-tree sidebar and editor area
 * (toolbar + CodeMirror editor).
 *
 * State management:
 * - selectedProjectId / selectedDocId live in URL params so deep-links work.
 * - Document content is fetched on doc selection and kept in local state.
 * - Autosave writes back via PUT with optimistic-concurrency token.
 */

import { FilePlus, RefreshCw, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { DocumentEditor } from "@/components/knowledge/DocumentEditor";
import {
	CreateDocumentDialog,
	DocumentToolbar,
} from "@/components/knowledge/DocumentToolbar";
import { DocumentTree } from "@/components/knowledge/DocumentTree";
import { ProjectPicker } from "@/components/knowledge/ProjectPicker";
import { knowledgeClient } from "@/services/knowledge";
import type { DocumentContent, TreeNode } from "@/types/knowledge";
import type { Project } from "@/types/orchestrator";

export function KnowledgebaseView() {
	const { projectId } = useParams<{ projectId?: string }>();
	const navigate = useNavigate();

	const [tree, setTree] = useState<TreeNode[]>([]);
	const [treeLoading, setTreeLoading] = useState(false);
	const [treeRefreshing, setTreeRefreshing] = useState(false);
	const [selectedDocId, setSelectedDocId] = useState<string | null>(null);
	const [docContent, setDocContent] = useState<DocumentContent | null>(null);
	const [saving, setSaving] = useState(false);
	const [showCreate, setShowCreate] = useState(false);
	const [error, setError] = useState<string | null>(null);

	// ------------------------------------------------------------------
	// Load tree when project changes
	// ------------------------------------------------------------------

	useEffect(() => {
		if (!projectId) {
			setTree([]);
			setDocContent(null);
			setSelectedDocId(null);
			return;
		}
		let cancelled = false;
		setTreeLoading(true);
		setError(null);
		knowledgeClient
			.getTree(projectId)
			.then((nodes) => {
				if (!cancelled) setTree(nodes);
			})
			.catch((e) => {
				if (!cancelled) setError(String(e));
			})
			.finally(() => {
				if (!cancelled) setTreeLoading(false);
			});
		return () => {
			cancelled = true;
		};
	}, [projectId]);

	// ------------------------------------------------------------------
	// Load document content when selection changes
	// ------------------------------------------------------------------

	useEffect(() => {
		if (!projectId || !selectedDocId) {
			setDocContent(null);
			return;
		}
		let cancelled = false;
		knowledgeClient
			.getDocumentContent(projectId, selectedDocId)
			.then((dc) => {
				if (!cancelled) setDocContent(dc);
			})
			.catch((e) => {
				if (!cancelled) setError(String(e));
			});
		return () => {
			cancelled = true;
		};
	}, [projectId, selectedDocId]);

	// ------------------------------------------------------------------
	// Autosave handler (called by DocumentEditor with debounced content)
	// ------------------------------------------------------------------

	const handleSave = useCallback(
		async (content: string, expectedUpdatedAt: string) => {
			if (!projectId || !selectedDocId) return;
			try {
				const updated = await knowledgeClient.updateDocument(
					projectId,
					selectedDocId,
					{ content, expected_updated_at: expectedUpdatedAt },
				);
				// Patch the local doc so updated_at stays current for next save
				setDocContent((prev) =>
					prev ? { ...prev, document: updated, content } : null,
				);
			} catch (e) {
				setError(`Autosave failed: ${e}`);
			} finally {
				setSaving(false);
			}
		},
		[projectId, selectedDocId],
	);

	// ------------------------------------------------------------------
	// Create document
	// ------------------------------------------------------------------

	const handleCreate = useCallback(
		async (relPath: string, title: string) => {
			if (!projectId) return;
			setShowCreate(false);
			try {
				const doc = await knowledgeClient.createDocument(projectId, {
					rel_path: relPath,
					title: title || undefined,
					content: `# ${title || relPath}\n`,
				});
				// Refresh tree and select the new doc
				const nodes = await knowledgeClient.getTree(projectId);
				setTree(nodes);
				setSelectedDocId(doc.id);
			} catch (e) {
				setError(`Failed to create document: ${e}`);
			}
		},
		[projectId],
	);

	// ------------------------------------------------------------------
	// Delete document
	// ------------------------------------------------------------------

	const handleDelete = useCallback(async () => {
		if (!projectId || !selectedDocId) return;
		if (!window.confirm("Permanently delete this document?")) return;
		try {
			await knowledgeClient.deleteDocument(projectId, selectedDocId);
			const nodes = await knowledgeClient.getTree(projectId);
			setTree(nodes);
			setSelectedDocId(null);
			setDocContent(null);
		} catch (e) {
			setError(`Failed to delete document: ${e}`);
		}
	}, [projectId, selectedDocId]);

	// ------------------------------------------------------------------
	// Project selection
	// ------------------------------------------------------------------

	function handleSelectProject(p: Project) {
		setSelectedDocId(null);
		setDocContent(null);
		navigate(`/knowledge/${p.id}`);
	}

	// ------------------------------------------------------------------
	// Refresh the document tree without flipping the full-page loader
	// ------------------------------------------------------------------

	const handleRefreshTree = useCallback(async () => {
		if (!projectId) return;
		setTreeRefreshing(true);
		setError(null);
		try {
			setTree(await knowledgeClient.getTree(projectId));
		} catch (e) {
			setError(String(e));
		} finally {
			setTreeRefreshing(false);
		}
	}, [projectId]);

	// ------------------------------------------------------------------
	// Render
	// ------------------------------------------------------------------

	return (
		<div className="space-y-6">
			{/* Page header */}
			<div className="flex items-start justify-between gap-4 flex-wrap">
				<div>
					<h1 className="text-2xl font-semibold text-th-text">Knowledgebase</h1>
					<p className="mt-1 text-sm text-th-text-muted">
						Browse and edit project documentation.
					</p>
				</div>

				<div className="flex items-center gap-2">
					{/* Project context picker */}
					<div className="w-56">
						<ProjectPicker
							selectedId={projectId ?? null}
							onSelect={handleSelectProject}
							onError={setError}
						/>
					</div>

					{/* New document */}
					<button
						type="button"
						onClick={() => setShowCreate(true)}
						disabled={!projectId}
						className="flex items-center gap-1.5 rounded-md bg-th-accent px-4 py-2 text-sm font-medium text-th-accent-text hover:bg-th-accent-hover transition-colors disabled:cursor-not-allowed disabled:opacity-50"
					>
						<FilePlus size={16} aria-hidden="true" />
						New Document
					</button>

					{/* Refresh tree */}
					<button
						type="button"
						onClick={handleRefreshTree}
						disabled={!projectId}
						aria-label="Refresh documents"
						className="rounded-md border border-th-border-strong bg-th-surface p-2 text-th-text-muted hover:bg-th-surface-hover hover:text-th-text-secondary transition-colors disabled:cursor-not-allowed disabled:opacity-50"
					>
						<RefreshCw
							size={16}
							className={treeRefreshing ? "animate-spin" : ""}
						/>
					</button>
				</div>
			</div>

			{/* Error banner */}
			{error && (
				<div
					role="alert"
					className="flex items-start gap-2 rounded-md border border-th-status-error-border bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text"
				>
					<span className="flex-1">{error}</span>
					<button
						type="button"
						onClick={() => setError(null)}
						aria-label="Dismiss error"
						className="shrink-0 rounded p-0.5 hover:bg-th-status-error-text/10 transition-colors"
					>
						<X size={14} />
					</button>
				</div>
			)}

			{/* Two-pane workspace */}
			<div className="flex h-[calc(100vh-13rem)] min-h-[420px] overflow-hidden rounded-lg border border-th-border bg-th-surface">
				{/* Left: document tree */}
				<aside className="flex w-60 shrink-0 flex-col border-r border-th-border bg-th-surface-sunken">
					<div className="border-b border-th-border px-4 py-2.5">
						<h2 className="text-xs font-semibold uppercase tracking-wide text-th-text-muted">
							Documents
						</h2>
					</div>
					<div className="flex-1 overflow-y-auto p-2">
						{!projectId ? (
							<p className="px-2 py-3 text-xs text-th-text-muted">
								Select a project to browse its documents.
							</p>
						) : treeLoading ? (
							<p className="px-2 py-3 text-xs text-th-text-muted">Loading…</p>
						) : (
							<DocumentTree
								nodes={tree}
								selectedDocId={selectedDocId}
								onSelectDoc={(docId) => setSelectedDocId(docId)}
							/>
						)}
					</div>
				</aside>

				{/* Right: editor */}
				<div className="flex flex-1 flex-col overflow-hidden">
					<DocumentToolbar
						document={docContent?.document ?? null}
						saving={saving}
						projectId={projectId ?? null}
						onDeleteClick={handleDelete}
					/>

					<div className="flex-1 overflow-hidden">
						<DocumentEditor
							docContent={docContent}
							onSave={handleSave}
							onSavingChange={setSaving}
						/>
					</div>
				</div>
			</div>

			{/* Create dialog */}
			{showCreate && (
				<CreateDocumentDialog
					onConfirm={handleCreate}
					onCancel={() => setShowCreate(false)}
				/>
			)}
		</div>
	);
}
