/**
 * KnowledgebaseView — two-panel layout: sidebar (project picker + document
 * tree) and main area (toolbar + CodeMirror editor).
 *
 * State management:
 * - selectedProjectId / selectedDocId live in URL params so deep-links work.
 * - Document content is fetched on doc selection and kept in local state.
 * - Autosave writes back via PUT with optimistic-concurrency token.
 */

import { useCallback, useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import type { DocumentContent, TreeNode } from "@/types/knowledge";
import type { Project } from "@/types/orchestrator";
import { knowledgeClient } from "@/services/knowledge";
import { DocumentEditor } from "@/components/knowledge/DocumentEditor";
import { DocumentTree } from "@/components/knowledge/DocumentTree";
import { DocumentToolbar, CreateDocumentDialog } from "@/components/knowledge/DocumentToolbar";
import { ProjectPicker } from "@/components/knowledge/ProjectPicker";

export function KnowledgebaseView() {
	const { projectId, "*": splat } = useParams<{
		projectId?: string;
		"*": string;
	}>();
	const navigate = useNavigate();

	const [project, setProject] = useState<Project | null>(null);
	const [tree, setTree] = useState<TreeNode[]>([]);
	const [treeLoading, setTreeLoading] = useState(false);
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
		setProject(p);
		setSelectedDocId(null);
		setDocContent(null);
		navigate(`/knowledge/${p.id}`);
	}

	// ------------------------------------------------------------------
	// Render
	// ------------------------------------------------------------------

	return (
		<div className="flex h-full overflow-hidden">
			{/* Left sidebar */}
			<aside className="flex w-56 shrink-0 flex-col border-r border-th-border bg-th-surface">
				{/* Project picker */}
				<div className="border-b border-th-border p-3">
					<ProjectPicker
						selectedId={projectId ?? null}
						onSelect={handleSelectProject}
					/>
				</div>

				{/* Document tree */}
				<div className="flex-1 overflow-y-auto p-2">
					{treeLoading ? (
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

			{/* Main editor area */}
			<div className="flex flex-1 flex-col overflow-hidden">
				{/* Error banner */}
				{error && (
					<div className="flex items-center gap-2 border-b border-th-status-error-text bg-th-status-error-bg px-4 py-2 text-xs text-th-status-error-text">
						<span className="flex-1">{error}</span>
						<button
							type="button"
							onClick={() => setError(null)}
							className="font-bold"
						>
							✕
						</button>
					</div>
				)}

				<DocumentToolbar
					document={docContent?.document ?? null}
					saving={saving}
					projectId={projectId ?? null}
					onCreateClick={() => setShowCreate(true)}
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
