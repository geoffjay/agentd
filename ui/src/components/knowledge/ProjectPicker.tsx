/**
 * ProjectPicker — dropdown that lists all projects from the orchestrator and
 * lets the user select one as the active knowledge context.
 */

import { ChevronDown, FolderOpen } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { Project } from "@/types/orchestrator";
import { orchestratorClient } from "@/services/orchestrator";

interface ProjectPickerProps {
	selectedId: string | null;
	onSelect: (project: Project) => void;
}

export function ProjectPicker({ selectedId, onSelect }: ProjectPickerProps) {
	const [projects, setProjects] = useState<Project[]>([]);
	const [loading, setLoading] = useState(true);
	const [open, setOpen] = useState(false);
	const ref = useRef<HTMLDivElement>(null);

	useEffect(() => {
		let cancelled = false;
		setLoading(true);
		orchestratorClient
			.listProjects({ limit: 200 })
			.then((page) => {
				if (!cancelled) setProjects(page.items);
			})
			.catch(console.error)
			.finally(() => {
				if (!cancelled) setLoading(false);
			});
		return () => {
			cancelled = true;
		};
	}, []);

	// Close on outside click
	useEffect(() => {
		function handle(e: MouseEvent) {
			if (ref.current && !ref.current.contains(e.target as Node)) {
				setOpen(false);
			}
		}
		document.addEventListener("mousedown", handle);
		return () => document.removeEventListener("mousedown", handle);
	}, []);

	const selected = projects.find((p) => p.id === selectedId);

	return (
		<div ref={ref} className="relative">
			<button
				type="button"
				onClick={() => setOpen((o) => !o)}
				className="flex w-full items-center gap-2 rounded-md border border-th-border bg-th-surface px-3 py-2 text-sm font-medium text-th-text hover:bg-th-surface-secondary"
			>
				<FolderOpen size={16} className="shrink-0 text-th-text-muted" />
				<span className="flex-1 truncate text-left">
					{loading
						? "Loading projects…"
						: selected
							? selected.name
							: "Select a project"}
				</span>
				<ChevronDown size={14} className="shrink-0 text-th-text-muted" />
			</button>

			{open && !loading && (
				<ul
					role="listbox"
					className="absolute z-50 mt-1 max-h-64 w-full overflow-y-auto rounded-md border border-th-border bg-th-surface shadow-lg"
				>
					{projects.length === 0 ? (
						<li className="px-3 py-2 text-sm text-th-text-muted">
							No projects found
						</li>
					) : (
						projects.map((p) => (
							<li key={p.id}>
								<button
									type="button"
									role="option"
									aria-selected={p.id === selectedId}
									onClick={() => {
										onSelect(p);
										setOpen(false);
									}}
									className={[
										"flex w-full items-center gap-2 px-3 py-2 text-sm hover:bg-th-surface-secondary",
										p.id === selectedId
											? "bg-th-surface-secondary font-medium text-th-text"
											: "text-th-text",
									].join(" ")}
								>
									<FolderOpen size={14} className="shrink-0 text-th-text-muted" />
									<span className="truncate">{p.name}</span>
								</button>
							</li>
						))
					)}
				</ul>
			)}
		</div>
	);
}
