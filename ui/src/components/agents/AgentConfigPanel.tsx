/**
 * AgentConfigPanel — collapsible panel showing agent configuration details.
 *
 * Displays:
 * - Working directory, shell, interactive mode
 * - System prompt (truncated, expandable)
 * - Tool policy (human-readable summary)
 * - Environment variables (values masked by default)
 * - Worktree info (if present)
 * - Model, tmux session
 * - Additional directories (with add/remove controls)
 */

import {
	ChevronDown,
	ChevronRight,
	Eye,
	EyeOff,
	FolderOpen,
	Plus,
	X,
} from "lucide-react";
import { useState } from "react";
import { HighlightedCode } from "@/components/common";
import type { Agent, ToolPolicy } from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// Tool policy display helper
// ---------------------------------------------------------------------------

function policyLabel(policy: ToolPolicy): string {
	switch (policy.mode) {
		case "allow_all":
			return "Allow All tools";
		case "deny_all":
			return "Deny All tools";
		case "require_approval":
			return "Require Approval for all tools";
		case "allow_list":
			return `Allow: ${policy.tools.length > 0 ? policy.tools.join(", ") : "(none)"}`;
		case "deny_list":
			return `Deny: ${policy.tools.length > 0 ? policy.tools.join(", ") : "(none)"}`;
	}
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function ConfigRow({
	label,
	children,
}: {
	label: string;
	children: React.ReactNode;
}) {
	return (
		<div className="flex flex-col gap-0.5 sm:flex-row sm:gap-4">
			<span className="w-36 flex-shrink-0 text-xs font-medium text-th-text-faint">
				{label}
			</span>
			<span className="text-sm text-th-text-secondary">
				{children}
			</span>
		</div>
	);
}

function EnvVarsRow({ env }: { env: Record<string, string> }) {
	const [revealed, setRevealed] = useState(false);
	const entries = Object.entries(env);
	if (entries.length === 0) return null;

	return (
		<div className="flex flex-col gap-1">
			<div className="flex items-center gap-2">
				<span className="text-xs font-medium text-th-text-faint">
					Environment
				</span>
				<button
					type="button"
					aria-label={revealed ? "Hide env values" : "Show env values"}
					onClick={() => setRevealed((v) => !v)}
					className="rounded p-0.5 text-th-text-muted hover:text-th-text-secondary"
				>
					{revealed ? <EyeOff size={13} /> : <Eye size={13} />}
				</button>
			</div>
			<div className="ml-0 flex flex-col gap-1 pl-0 font-mono text-xs">
				{entries.map(([key, value]) => (
					<div key={key} className="flex gap-2">
						<span className="text-th-text-muted">{key}=</span>
						<span className="text-th-text-secondary">
							{revealed ? value : "••••••••"}
						</span>
					</div>
				))}
			</div>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Launch command helpers
// ---------------------------------------------------------------------------

/**
 * Replace inline system prompt values in a launch command string with a
 * `<system prompt>` placeholder so the full (potentially long) prompt text
 * is not duplicated in the debug view.
 *
 * Handles both `--system-prompt` and `--append-system-prompt` flags.
 * File-path variants (`--system-prompt-file`, `--append-system-prompt-file`)
 * are left as-is since the path is short and not sensitive.
 *
 * The command builder shell-escapes the prompt using single-quote POSIX
 * escaping (`'value'` with embedded `'` replaced by `'\''`).
 */
function redactSystemPromptInCommand(
	command: string,
	systemPrompt: string | undefined | null,
): string {
	if (!systemPrompt) return command;
	const escaped = systemPrompt.replace(/'/g, "'\\''");
	return command
		.replace(
			`--system-prompt '${escaped}'`,
			"--system-prompt '<system prompt>'",
		)
		.replace(
			`--append-system-prompt '${escaped}'`,
			"--append-system-prompt '<system prompt>'",
		);
}

// ---------------------------------------------------------------------------
// System prompt row
// ---------------------------------------------------------------------------

function SystemPromptRow({ prompt }: { prompt: string }) {
	const [expanded, setExpanded] = useState(false);
	const isLong = prompt.length > 200;

	return (
		<div className="flex flex-col gap-0.5 sm:flex-row sm:gap-4">
			<span className="w-36 flex-shrink-0 text-xs font-medium text-th-text-faint">
				System Prompt
			</span>
			<div className="flex flex-col gap-1 min-w-0 flex-1">
				{expanded ? (
					<HighlightedCode
						code={prompt}
						language="markdown"
						maxHeight="20rem"
						className="border border-th-border"
					/>
				) : (
					<p className="whitespace-pre-wrap text-sm text-th-text-secondary">
						{isLong ? `${prompt.slice(0, 200)}…` : prompt}
					</p>
				)}
				{isLong && (
					<button
						type="button"
						onClick={() => setExpanded((e) => !e)}
						className="self-start text-xs text-th-text-link hover:opacity-80"
					>
						{expanded ? "Show less" : "Show more"}
					</button>
				)}
			</div>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Add Directory dialog (inline)
// ---------------------------------------------------------------------------

interface AddDirDialogProps {
	open: boolean;
	saving: boolean;
	error?: string;
	onConfirm: (path: string) => void;
	onCancel: () => void;
}

function AddDirDialog({
	open,
	saving,
	error,
	onConfirm,
	onCancel,
}: AddDirDialogProps) {
	const [path, setPath] = useState("");

	if (!open) return null;

	function handleSubmit() {
		const trimmed = path.trim();
		if (trimmed) {
			onConfirm(trimmed);
		}
	}

	function handleKeyDown(e: React.KeyboardEvent) {
		if (e.key === "Enter") handleSubmit();
		if (e.key === "Escape") onCancel();
	}

	const inputCls =
		"block w-full rounded-md border border-th-border-input bg-th-input px-3 py-2 text-sm text-th-text font-mono focus:border-th-border-focus focus:outline-none focus:ring-1 focus:ring-th-focus-ring";

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center p-4">
			<div
				className="absolute inset-0 bg-th-overlay"
				aria-hidden="true"
				onClick={onCancel}
			/>
			<div
				role="dialog"
				aria-modal="true"
				aria-labelledby="add-dir-title"
				className="relative rounded-lg bg-th-surface p-6 shadow-xl"
			>
				<h2
					id="add-dir-title"
					className="mb-4 text-base font-semibold text-th-text"
				>
					Add Directory
				</h2>

				<p className="mb-3 text-sm text-th-text-muted">
					Enter an absolute path to grant the agent access via{" "}
					<code className="rounded bg-th-surface-sunken px-1 py-0.5 font-mono text-xs">
						--add-dir
					</code>
					.
				</p>

				{error && (
					<p
						role="alert"
						className="mb-3 text-sm text-th-status-error-text"
					>
						{error}
					</p>
				)}

				<div className="flex flex-col gap-2">
					<label
						htmlFor="add-dir-path"
						className="text-sm font-medium text-th-text-secondary"
					>
						Directory path
					</label>
					<input
						id="add-dir-path"
						type="text"
						value={path}
						onChange={(e) => setPath(e.target.value)}
						onKeyDown={handleKeyDown}
						placeholder="/path/to/directory"
						className={inputCls}
						autoFocus
						disabled={saving}
					/>
				</div>

				<p className="mt-3 text-xs text-th-status-warning-text">
					Directory changes take effect on the next agent restart.
				</p>

				<div className="mt-5 flex justify-end gap-2">
					<button
						type="button"
						onClick={onCancel}
						disabled={saving}
						className="rounded-md border border-th-border-input bg-th-surface px-4 py-2 text-sm font-medium text-th-text-secondary hover:bg-th-surface-hover disabled:opacity-50"
					>
						Cancel
					</button>
					<button
						type="button"
						onClick={handleSubmit}
						disabled={saving || !path.trim()}
						className="rounded-md bg-th-accent px-4 py-2 text-sm font-medium text-th-accent-text hover:bg-th-accent-hover disabled:opacity-50 transition-colors"
					>
						{saving ? "Adding…" : "Add Directory"}
					</button>
				</div>
			</div>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Additional Directories row
// ---------------------------------------------------------------------------

interface AdditionalDirsRowProps {
	dirs: string[];
	onAdd?: (path: string) => Promise<void>;
	onRemove?: (path: string) => Promise<void>;
}

function AdditionalDirsRow({ dirs, onAdd, onRemove }: AdditionalDirsRowProps) {
	const [showDialog, setShowDialog] = useState(false);
	const [saving, setSaving] = useState(false);
	const [addError, setAddError] = useState<string | undefined>();
	const [removingPath, setRemovingPath] = useState<string | undefined>();
	const [restartNotice, setRestartNotice] = useState(false);

	async function handleAdd(path: string) {
		if (!onAdd) return;
		setSaving(true);
		setAddError(undefined);
		try {
			await onAdd(path);
			setShowDialog(false);
			setRestartNotice(true);
			setTimeout(() => setRestartNotice(false), 5000);
		} catch (err) {
			setAddError(
				err instanceof Error ? err.message : "Failed to add directory",
			);
		} finally {
			setSaving(false);
		}
	}

	async function handleRemove(path: string) {
		if (!onRemove) return;
		setRemovingPath(path);
		try {
			await onRemove(path);
			setRestartNotice(true);
			setTimeout(() => setRestartNotice(false), 5000);
		} catch {
			// Silently ignore — the list will not update on error
		} finally {
			setRemovingPath(undefined);
		}
	}

	const canEdit = Boolean(onAdd && onRemove);

	return (
		<div className="flex flex-col gap-1">
			<div className="flex items-center gap-2">
				<span className="w-36 flex-shrink-0 text-xs font-medium text-th-text-faint">
					Additional Dirs
				</span>
				{canEdit && (
					<button
						type="button"
						aria-label="Add directory"
						onClick={() => {
							setAddError(undefined);
							setShowDialog(true);
						}}
						className="flex items-center gap-1 rounded p-0.5 text-xs text-th-text-link hover:opacity-80"
					>
						<Plus size={13} />
						Add
					</button>
				)}
			</div>

			{dirs.length === 0 ? (
				<span className="pl-0 text-sm text-th-text-faint sm:pl-40">
					(none)
				</span>
			) : (
				<ul className="flex flex-col gap-1 pl-0 sm:pl-40">
					{dirs.map((dir) => (
						<li key={dir} className="flex items-center gap-2 group">
							<FolderOpen
								size={13}
								className="flex-shrink-0 text-th-text-faint"
								aria-hidden="true"
							/>
							<span className="flex-1 font-mono text-xs text-th-text-secondary break-all">
								{dir}
							</span>
							{canEdit && (
								<button
									type="button"
									aria-label={`Remove directory ${dir}`}
									onClick={() => handleRemove(dir)}
									disabled={removingPath === dir}
									className="rounded p-0.5 text-th-text-faint hover:text-th-status-error-text disabled:opacity-50 opacity-0 group-hover:opacity-100 transition-opacity"
								>
									<X size={13} />
								</button>
							)}
						</li>
					))}
				</ul>
			)}

			{restartNotice && (
				<p className="pl-0 text-xs text-th-status-warning-text sm:pl-40">
					Directory changes take effect on the next agent restart.
				</p>
			)}

			<AddDirDialog
				open={showDialog}
				saving={saving}
				error={addError}
				onConfirm={handleAdd}
				onCancel={() => setShowDialog(false)}
			/>
		</div>
	);
}

// ---------------------------------------------------------------------------
// AgentConfigPanel
// ---------------------------------------------------------------------------

export interface AgentConfigPanelProps {
	agent: Agent;
	onAddDir?: (path: string) => Promise<void>;
	onRemoveDir?: (path: string) => Promise<void>;
}

export function AgentConfigPanel({
	agent,
	onAddDir,
	onRemoveDir,
}: AgentConfigPanelProps) {
	const [open, setOpen] = useState(true);
	const { config } = agent;

	return (
		<section
			aria-label="Agent configuration"
			className="rounded-lg border border-th-border bg-th-surface"
		>
			{/* Header / toggle */}
			<button
				type="button"
				aria-expanded={open}
				aria-controls="agent-config-body"
				onClick={() => setOpen((o) => !o)}
				className="flex w-full items-center justify-between px-4 py-3 text-sm font-medium text-th-text hover:bg-th-surface-hover"
			>
				<span>Configuration</span>
				{open ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
			</button>

			{open && (
				<div
					id="agent-config-body"
					className="flex flex-col gap-3 border-t border-th-border px-4 py-4"
				>
					<ConfigRow label="Working Dir">
						<span className="font-mono text-xs">{config.working_dir}</span>
					</ConfigRow>

					<ConfigRow label="Shell">
						<span className="font-mono text-xs">{config.shell}</span>
					</ConfigRow>

					<ConfigRow label="Interactive">
						{config.interactive ? (
							<span className="text-th-status-success-text">
								Yes (TTY)
							</span>
						) : (
							<span className="text-th-text-muted">No</span>
						)}
					</ConfigRow>

					{config.model && (
						<ConfigRow label="Model">
							<span className="font-mono text-xs">{config.model}</span>
						</ConfigRow>
					)}

					{config.worktree && (
						<ConfigRow label="Worktree">
							<span className="font-mono text-xs">{config.worktree}</span>
						</ConfigRow>
					)}

					{agent.session_id && (
						<ConfigRow label="Session">
							<span className="font-mono text-xs">{agent.session_id}</span>
						</ConfigRow>
					)}

					{agent.pid != null && (
						<ConfigRow label="PID">
							<span className="font-mono text-xs">{agent.pid}</span>
						</ConfigRow>
					)}

					<ConfigRow label="Tool Policy">
						{policyLabel(config.tool_policy)}
					</ConfigRow>

					<ConfigRow label="Auto-clear">
						{config.auto_clear_threshold != null &&
						config.auto_clear_threshold > 0 ? (
							<span className="text-th-status-warning-text">
								at {config.auto_clear_threshold.toLocaleString()} tokens
							</span>
						) : (
							<span className="text-th-text-muted">Disabled</span>
						)}
					</ConfigRow>

					{config.system_prompt && (
						<SystemPromptRow prompt={config.system_prompt} />
					)}

					{config.system_prompt_file && (
						<ConfigRow label="Prompt File">
							<span className="font-mono text-xs break-all">
								{config.system_prompt_file}
							</span>
						</ConfigRow>
					)}

					{(config.system_prompt || config.system_prompt_file) && (
						<ConfigRow label="Prompt Mode">
							{config.append_system_prompt ? (
								<span className="text-th-status-info-text">Append</span>
							) : (
								<span className="text-th-text-muted">
									Replace
								</span>
							)}
						</ConfigRow>
					)}

					{config.env && Object.keys(config.env).length > 0 && (
						<EnvVarsRow env={config.env} />
					)}

					<AdditionalDirsRow
						dirs={config.additional_dirs ?? []}
						onAdd={onAddDir}
						onRemove={onRemoveDir}
					/>

					{agent.launch_command && (
						<div className="flex flex-col gap-1">
							<span className="text-xs font-medium text-th-text-faint">
								Launch Command
							</span>
							<div data-testid="launch-command-code">
								<HighlightedCode
									code={redactSystemPromptInCommand(
										agent.launch_command,
										config.system_prompt,
									)}
									language="bash"
									maxHeight="12rem"
									className="border border-th-border"
								/>
							</div>
						</div>
					)}
				</div>
			)}
		</section>
	);
}

export default AgentConfigPanel;
