/**
 * DocumentTree — renders the virtual folder/file tree for a project.
 *
 * Folders are expandable; clicking a file node fires onSelectDoc.
 */

import { ChevronDown, ChevronRight, File, FolderOpen } from "lucide-react";
import { useState } from "react";
import type { TreeNode } from "@/types/knowledge";

interface DocumentTreeProps {
	nodes: TreeNode[];
	selectedDocId: string | null;
	onSelectDoc: (docId: string, path: string) => void;
}

export function DocumentTree({
	nodes,
	selectedDocId,
	onSelectDoc,
}: DocumentTreeProps) {
	if (nodes.length === 0) {
		return (
			<p className="px-2 py-3 text-xs text-th-text-muted">
				No documents yet. Create one to get started.
			</p>
		);
	}
	return (
		<ul role="tree" className="space-y-0.5">
			{nodes.map((node) => (
				<TreeItem
					key={node.path}
					node={node}
					selectedDocId={selectedDocId}
					onSelectDoc={onSelectDoc}
					depth={0}
				/>
			))}
		</ul>
	);
}

interface TreeItemProps {
	node: TreeNode;
	selectedDocId: string | null;
	onSelectDoc: (docId: string, path: string) => void;
	depth: number;
}

function TreeItem({ node, selectedDocId, onSelectDoc, depth }: TreeItemProps) {
	const [expanded, setExpanded] = useState(true);
	const indent = depth * 12;

	if (node.type === "folder") {
		return (
			<li role="treeitem" aria-expanded={expanded}>
				<button
					type="button"
					onClick={() => setExpanded((e) => !e)}
					className="flex w-full items-center gap-1 rounded px-2 py-1 text-xs font-medium text-th-text-muted hover:bg-th-surface-secondary"
					style={{ paddingLeft: `${8 + indent}px` }}
				>
					{expanded ? (
						<ChevronDown size={12} className="shrink-0" />
					) : (
						<ChevronRight size={12} className="shrink-0" />
					)}
					<FolderOpen size={13} className="shrink-0" />
					<span className="truncate">{node.name}</span>
				</button>
				{expanded && (
					<ul role="group">
						{node.children.map((child) => (
							<TreeItem
								key={child.path}
								node={child}
								selectedDocId={selectedDocId}
								onSelectDoc={onSelectDoc}
								depth={depth + 1}
							/>
						))}
					</ul>
				)}
			</li>
		);
	}

	const isSelected = node.doc_id === selectedDocId;
	return (
		<li role="treeitem" aria-selected={isSelected}>
			<button
				type="button"
				onClick={() => onSelectDoc(node.doc_id, node.path)}
				className={[
					"flex w-full items-center gap-1 rounded px-2 py-1 text-xs",
					isSelected
						? "bg-th-nav-active font-medium text-th-text-nav-active"
						: "text-th-text hover:bg-th-surface-secondary",
				].join(" ")}
				style={{ paddingLeft: `${8 + indent}px` }}
			>
				<File size={12} className="shrink-0" />
				<span className="truncate">{node.name}</span>
			</button>
		</li>
	);
}
