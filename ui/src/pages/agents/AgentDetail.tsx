/**
 * AgentDetail — detail view for a single agent.
 *
 * Layout:
 * ┌─ Header: name, status, ID, timestamps, model, actions ────────────────┐
 * │  ┌─ Main (log + command) ──────────────┐  ┌─ Sidebar ───────────────┐ │
 * │  │  AgentLogView                       │  │  AgentConfigPanel       │ │
 * │  │  AgentCommandInput                  │  │  Tool Policy            │ │
 * │  └─────────────────────────────────────┘  │  Pending Approvals      │ │
 * │                                           └─────────────────────────┘ │
 * └───────────────────────────────────────────────────────────────────────┘
 */

import {
	ArrowLeft,
	ChevronDown,
	Copy,
	Eraser,
	FolderPlus,
	Loader2,
	MoreHorizontal,
	RefreshCw,
	Settings2,
	Trash2,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { AgentApprovals } from "@/components/agents/AgentApprovals";
import { AgentCommandInput } from "@/components/agents/AgentCommandInput";
import { AgentConfigPanel } from "@/components/agents/AgentConfigPanel";
import { AgentLogView } from "@/components/agents/AgentLogView";
import { AgentPolicyEditor } from "@/components/agents/AgentPolicyEditor";
import { AgentStatusBadge } from "@/components/agents/AgentStatusBadge";
import { AgentTerminal } from "@/components/agents/AgentTerminal";
import { AgentTodosPanel } from "@/components/agents/AgentTodosPanel";
import { AgentUsagePanel } from "@/components/agents/AgentUsagePanel";
import { PolicyDisplay } from "@/components/agents/PolicyDisplay";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { CardSkeleton } from "@/components/common/LoadingSkeleton";
import { useAgentDetail } from "@/hooks/useAgentDetail";
import { useAgentStream } from "@/hooks/useAgentStream";
import { useAgentUsage } from "@/hooks/useAgentUsage";
import { useToast } from "@/hooks/useToast";
import { orchestratorClient } from "@/services/orchestrator";
import type { BackendInfo } from "@/types/common";
import type {
	SessionUsage,
	SetModelRequest,
	ToolPolicy,
} from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// Model selector constants
// ---------------------------------------------------------------------------

const MODELS = [
	{ label: "Default (server)", value: "" },
	{ label: "Claude Sonnet 4.6", value: "claude-sonnet-4-6" },
	{ label: "Claude Sonnet 4.6 (1M Context)", value: "claude-sonnet-4-6[1m]" },
	{ label: "Claude Opus 4.6", value: "claude-opus-4-6" },
	{ label: "Claude Opus 4.6 (1M Context)", value: "claude-opus-4-6[1m]" },
	{ label: "Claude Haiku 4.6", value: "claude-haiku-4-6" },
];

// ---------------------------------------------------------------------------
// Change Model dialog
// ---------------------------------------------------------------------------

interface ChangeModelDialogProps {
	open: boolean;
	currentModel?: string;
	onSave: (request: SetModelRequest) => Promise<void>;
	onClose: () => void;
}

function ChangeModelDialog({
	open,
	currentModel,
	onSave,
	onClose,
}: ChangeModelDialogProps) {
	const [model, setModel] = useState(currentModel ?? "");
	const [restart, setRestart] = useState(true);
	const [saving, setSaving] = useState(false);
	const [error, setError] = useState<string | undefined>();

	if (!open) return null;

	async function handleSave() {
		setSaving(true);
		setError(undefined);
		try {
			await onSave({ model: model || undefined, restart });
			onClose();
		} catch (err) {
			setError(err instanceof Error ? err.message : "Failed to update model");
		} finally {
			setSaving(false);
		}
	}

	const inputCls =
		"block w-full rounded-md border border-th-border-input bg-th-input px-3 py-2 text-sm text-th-text focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-th-focus-ring";

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center p-4">
			<div
				className="absolute inset-0 bg-th-overlay"
				aria-hidden="true"
				onClick={onClose}
			/>
			<div
				role="dialog"
				aria-modal="true"
				aria-labelledby="change-model-title"
				className="relative rounded-lg bg-th-surface p-6 shadow-xl"
			>
				<h2
					id="change-model-title"
					className="mb-4 text-base font-semibold text-th-text"
				>
					Change Model
				</h2>

				{error && (
					<p role="alert" className="mb-3 text-sm text-th-status-error-text">
						{error}
					</p>
				)}

				<div className="flex flex-col gap-3">
					<div>
						<label
							htmlFor="change-model-select"
							className="mb-1 block text-sm font-medium text-th-text-secondary"
						>
							Model
						</label>
						<select
							id="change-model-select"
							value={model}
							onChange={(e) => setModel(e.target.value)}
							className={inputCls}
						>
							{MODELS.map((m) => (
								<option key={m.value} value={m.value}>
									{m.label}
								</option>
							))}
						</select>
					</div>

					<div className="flex items-center gap-2">
						<input
							id="change-model-restart"
							type="checkbox"
							checked={restart}
							onChange={(e) => setRestart(e.target.checked)}
							className="h-4 w-4 rounded border-th-border-strong text-th-accent"
						/>
						<label
							htmlFor="change-model-restart"
							className="text-sm text-th-text-secondary"
						>
							Restart agent after change
						</label>
					</div>
				</div>

				<div className="mt-5 flex justify-end gap-2">
					<button
						type="button"
						onClick={onClose}
						disabled={saving}
						className="rounded-md border border-th-border-strong bg-th-surface px-4 py-2 text-sm font-medium text-th-text-secondary hover:bg-th-surface-hover disabled:opacity-50"
					>
						Cancel
					</button>
					<button
						type="button"
						onClick={handleSave}
						disabled={saving}
						className="rounded-md bg-th-accent px-4 py-2 text-sm font-medium text-th-accent-text hover:bg-th-accent-hover disabled:opacity-50 transition-colors"
					>
						{saving ? "Saving…" : "Save"}
					</button>
				</div>
			</div>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Clear Context dialog — shows session summary before clearing
// ---------------------------------------------------------------------------

const costFmt = new Intl.NumberFormat("en-US", {
	style: "currency",
	currency: "USD",
	minimumFractionDigits: 4,
	maximumFractionDigits: 4,
});

interface ClearContextDialogProps {
	open: boolean;
	session?: SessionUsage;
	loading: boolean;
	onConfirm: () => void;
	onCancel: () => void;
}

function ClearContextDialog({
	open,
	session,
	loading,
	onConfirm,
	onCancel,
}: ClearContextDialogProps) {
	if (!open) return null;

	const totalTokens = session
		? session.input_tokens +
			session.output_tokens +
			session.cache_read_input_tokens +
			session.cache_creation_input_tokens
		: 0;

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center p-4">
			<div
				className="absolute inset-0 bg-th-overlay"
				aria-hidden="true"
				onClick={onCancel}
			/>
			<div
				role="alertdialog"
				aria-modal="true"
				aria-labelledby="clear-context-title"
				aria-describedby="clear-context-desc"
				className="relative rounded-lg bg-th-surface p-6 shadow-xl"
			>
				<h2
					id="clear-context-title"
					className="text-base font-semibold text-th-text"
				>
					Clear context?
				</h2>

				<p id="clear-context-desc" className="mt-2 text-sm text-th-text-muted">
					This will clear the agent&apos;s current context and start a new
					session. Current session usage will be saved.
				</p>

				{/* Session stats summary */}
				{session && totalTokens > 0 && (
					<div className="mt-3 rounded-md bg-th-surface-sunken p-3">
						<p className="mb-1.5 text-xs font-medium text-th-text-muted">
							Current session
						</p>
						<div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
							<span className="text-th-text-muted">Input tokens</span>
							<span className="text-right font-medium text-th-text-secondary">
								{session.input_tokens.toLocaleString()}
							</span>
							<span className="text-th-text-muted">Output tokens</span>
							<span className="text-right font-medium text-th-text-secondary">
								{session.output_tokens.toLocaleString()}
							</span>
							<span className="text-th-text-muted">Cache tokens</span>
							<span className="text-right font-medium text-th-text-secondary">
								{(
									session.cache_read_input_tokens +
									session.cache_creation_input_tokens
								).toLocaleString()}
							</span>
							<span className="text-th-text-muted">Cost</span>
							<span className="text-right font-medium text-th-text-secondary">
								{costFmt.format(session.total_cost_usd)}
							</span>
							<span className="text-th-text-muted">Turns</span>
							<span className="text-right font-medium text-th-text-secondary">
								{session.num_turns}
							</span>
						</div>
					</div>
				)}

				<div className="mt-5 flex justify-end gap-3">
					<button
						type="button"
						onClick={onCancel}
						disabled={loading}
						className="rounded-md border border-th-border-strong bg-th-surface px-4 py-2 text-sm font-medium text-th-text-secondary hover:bg-th-surface-hover focus:outline-none focus:ring-2 focus:ring-th-focus-ring focus:ring-offset-2 disabled:opacity-50"
					>
						Cancel
					</button>
					<button
						type="button"
						onClick={onConfirm}
						disabled={loading}
						className="rounded-md bg-th-status-warning-dot px-4 py-2 text-sm font-medium text-th-accent-text hover:opacity-90 focus:outline-none focus:ring-2 focus:ring-th-focus-ring focus:ring-offset-2 disabled:opacity-50 transition-colors"
					>
						{loading ? "Clearing…" : "Clear Context"}
					</button>
				</div>
			</div>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Add Directory dialog
// ---------------------------------------------------------------------------

interface AddDirDialogProps {
	open: boolean;
	onConfirm: (path: string) => Promise<void>;
	onClose: () => void;
}

function AddDirDialog({ open, onConfirm, onClose }: AddDirDialogProps) {
	const [path, setPath] = useState("");
	const [saving, setSaving] = useState(false);
	const [error, setError] = useState<string | undefined>();

	if (!open) return null;

	async function handleSubmit() {
		const trimmed = path.trim();
		if (!trimmed) return;
		setSaving(true);
		setError(undefined);
		try {
			await onConfirm(trimmed);
			setPath("");
			onClose();
		} catch (err) {
			setError(err instanceof Error ? err.message : "Failed to add directory");
		} finally {
			setSaving(false);
		}
	}

	function handleKeyDown(e: React.KeyboardEvent) {
		if (e.key === "Enter") handleSubmit();
		if (e.key === "Escape") onClose();
	}

	const inputCls =
		"block w-full rounded-md border border-th-border-input bg-th-input px-3 py-2 font-mono text-sm text-th-text focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-th-focus-ring";

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center p-4">
			<div
				className="absolute inset-0 bg-th-overlay"
				aria-hidden="true"
				onClick={onClose}
			/>
			<div
				role="dialog"
				aria-modal="true"
				aria-labelledby="add-dir-header-title"
				className="relative w-full max-w-md rounded-lg bg-th-surface p-6 shadow-xl"
			>
				<h2
					id="add-dir-header-title"
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
					<p role="alert" className="mb-3 text-sm text-th-status-error-text">
						{error}
					</p>
				)}

				<div className="flex flex-col gap-2">
					<label
						htmlFor="add-dir-header-path"
						className="text-sm font-medium text-th-text-secondary"
					>
						Directory path
					</label>
					<input
						id="add-dir-header-path"
						type="text"
						value={path}
						onChange={(e) => setPath(e.target.value)}
						onKeyDown={handleKeyDown}
						placeholder="/path/to/directory"
						className={inputCls}
						// eslint-disable-next-line jsx-a11y/no-autofocus
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
						onClick={onClose}
						disabled={saving}
						className="rounded-md border border-th-border-strong bg-th-surface px-4 py-2 text-sm font-medium text-th-text-secondary hover:bg-th-surface-hover disabled:opacity-50"
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
// Actions dropdown menu
// ---------------------------------------------------------------------------

interface ActionsDropdownProps {
	isRunning: boolean;
	isFailed: boolean;
	clearing: boolean;
	restarting: boolean;
	onChangeModel: () => void;
	onAddDir: () => void;
	onClearContext: () => void;
	onRestart: () => void;
	onTerminate: () => void;
}

function ActionsDropdown({
	isRunning,
	isFailed,
	clearing,
	restarting,
	onChangeModel,
	onAddDir,
	onClearContext,
	onRestart,
	onTerminate,
}: ActionsDropdownProps) {
	const [open, setOpen] = useState(false);
	const menuRef = useRef<HTMLDivElement>(null);
	const buttonRef = useRef<HTMLButtonElement>(null);

	// Close on outside click
	useEffect(() => {
		if (!open) return;
		function handlePointerDown(e: MouseEvent) {
			if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
				setOpen(false);
			}
		}
		document.addEventListener("mousedown", handlePointerDown);
		return () => document.removeEventListener("mousedown", handlePointerDown);
	}, [open]);

	// Escape to close, arrow keys to navigate
	useEffect(() => {
		if (!open) return;
		function handleKeyDown(e: KeyboardEvent) {
			if (e.key === "Escape") {
				setOpen(false);
				buttonRef.current?.focus();
				return;
			}
			if (e.key === "ArrowDown" || e.key === "ArrowUp") {
				e.preventDefault();
				const items = Array.from(
					menuRef.current?.querySelectorAll<HTMLElement>(
						'[role="menuitem"]:not([disabled])',
					) ?? [],
				);
				if (items.length === 0) return;
				const idx = items.indexOf(document.activeElement as HTMLElement);
				if (e.key === "ArrowDown") {
					items[(idx + 1) % items.length].focus();
				} else {
					items[(idx - 1 + items.length) % items.length].focus();
				}
			}
		}
		document.addEventListener("keydown", handleKeyDown);
		return () => document.removeEventListener("keydown", handleKeyDown);
	}, [open]);

	function pick(fn: () => void) {
		setOpen(false);
		fn();
	}

	const itemCls =
		"flex w-full items-center gap-2.5 px-3 py-2 text-left text-sm focus:outline-none";
	const normalItem = `${itemCls} text-th-text-secondary hover:bg-th-surface-hover focus:bg-th-surface-hover`;
	const amberItem = `${itemCls} text-th-status-warning-text hover:bg-th-status-warning-bg focus:bg-th-status-warning-bg disabled:opacity-50 disabled:cursor-not-allowed`;
	const redItem = `${itemCls} text-th-status-error-text hover:bg-th-status-error-bg focus:bg-th-status-error-bg`;

	return (
		<div ref={menuRef} className="relative">
			<button
				ref={buttonRef}
				type="button"
				aria-haspopup="menu"
				aria-expanded={open}
				onClick={() => setOpen((o) => !o)}
				className="flex items-center gap-1.5 rounded-md border border-th-border-strong bg-th-surface px-3 py-1.5 text-sm font-medium text-th-text-secondary hover:bg-th-surface-hover focus:outline-none focus:ring-2 focus:ring-th-focus-ring focus:ring-offset-1"
			>
				<MoreHorizontal size={14} aria-hidden="true" />
				<span className="hidden sm:inline">Actions</span>
				<ChevronDown
					size={12}
					aria-hidden="true"
					className={`transition-transform ${open ? "rotate-180" : ""}`}
				/>
			</button>

			{open && (
				<div
					role="menu"
					aria-label="Agent actions"
					className="absolute right-0 z-20 mt-1 w-52 rounded-md border border-th-border bg-th-surface py-1 shadow-lg"
				>
					{/* Change Model */}
					<button
						role="menuitem"
						type="button"
						onClick={() => pick(onChangeModel)}
						className={normalItem}
					>
						<Settings2
							size={14}
							className="text-th-text-faint"
							aria-hidden="true"
						/>
						Change Model
					</button>

					{/* Add Directory */}
					<button
						role="menuitem"
						type="button"
						onClick={() => pick(onAddDir)}
						className={normalItem}
					>
						<FolderPlus
							size={14}
							className="text-th-text-faint"
							aria-hidden="true"
						/>
						Add Directory
					</button>

					{/* Clear Context (amber / warning) */}
					<button
						role="menuitem"
						type="button"
						onClick={() => pick(onClearContext)}
						disabled={!isRunning || clearing}
						className={amberItem}
					>
						{clearing ? (
							<Loader2
								size={14}
								className="animate-spin text-th-status-warning-text"
								aria-hidden="true"
							/>
						) : (
							<Eraser
								size={14}
								className="text-th-status-warning-text"
								aria-hidden="true"
							/>
						)}
						Clear Context
					</button>

					{/* Restart (shown for failed/stopped agents) */}
					{isFailed && (
						<button
							role="menuitem"
							type="button"
							onClick={() => pick(onRestart)}
							disabled={restarting}
							className={normalItem}
						>
							{restarting ? (
								<Loader2
									size={14}
									className="animate-spin text-th-text-faint"
									aria-hidden="true"
								/>
							) : (
								<RefreshCw
									size={14}
									className="text-th-text-faint"
									aria-hidden="true"
								/>
							)}
							Restart Agent
						</button>
					)}

					{/* Divider */}
					<div role="separator" className="my-1 border-t border-th-border" />

					{/* Terminate (red / danger) */}
					<button
						role="menuitem"
						type="button"
						onClick={() => pick(onTerminate)}
						className={redItem}
					>
						<Trash2
							size={14}
							className="text-th-status-error-text"
							aria-hidden="true"
						/>
						Terminate
					</button>
				</div>
			)}
		</div>
	);
}

// ---------------------------------------------------------------------------
// AgentDetail
// ---------------------------------------------------------------------------

export function AgentDetail() {
	const { id } = useParams<{ id: string }>();
	const navigate = useNavigate();

	const agentId = id ?? "";

	const {
		agent,
		loading,
		error,
		refetch,
		deleteAgent,
		sendMessage,
		updateModel,
		updatePolicy,
		approvals,
		approvalsLoading,
		approvalsError,
		approveRequest,
		denyRequest,
	} = useAgentDetail(agentId);

	const {
		lines,
		status: streamStatus,
		clear: clearLog,
	} = useAgentStream(agentId);
	const { usage, clearContext, clearing } = useAgentUsage(agentId);
	const toast = useToast();

	const [confirmTerminate, setConfirmTerminate] = useState(false);
	const [terminating, setTerminating] = useState(false);
	const [restarting, setRestarting] = useState(false);
	const [confirmClearContext, setConfirmClearContext] = useState(false);
	const [showModelDialog, setShowModelDialog] = useState(false);
	const [showAddDirDialog, setShowAddDirDialog] = useState(false);
	const [policyEditing, setPolicyEditing] = useState(false);
	const [copied, setCopied] = useState(false);
	const [activeTab, setActiveTab] = useState<"logs" | "terminal">("logs");
	// Backend capability detection — populated on mount; null means loading.
	const [backendInfo, setBackendInfo] = useState<BackendInfo | null>(null);

	// Fetch backend capabilities once on mount so the UI can show/hide features.
	useEffect(() => {
		orchestratorClient
			.getInfo()
			.then(setBackendInfo)
			.catch(() => {
				// If the /info endpoint is unavailable (older orchestrator), assume
				// a tmux backend without PTY streaming capability.
				setBackendInfo({
					backend_type: "tmux",
					version: "unknown",
					capabilities: [],
				});
			});
	}, []);

	// Derived: whether the active backend supports PTY terminal streaming.
	const ptyAvailable = backendInfo?.capabilities.includes("terminal") ?? false;

	// ---------------------------------------------------------------------------
	// Handlers
	// ---------------------------------------------------------------------------

	async function handleTerminate() {
		setTerminating(true);
		try {
			await deleteAgent();
			navigate("/agents");
		} catch {
			// Navigate anyway — the agent may already be gone
			navigate("/agents");
		} finally {
			setTerminating(false);
			setConfirmTerminate(false);
		}
	}

	async function handleRestart() {
		setRestarting(true);
		try {
			await orchestratorClient.restartAgent(agentId);
			toast.success("Agent restarted");
			refetch();
		} catch (e) {
			toast.error(`Failed to restart agent: ${e}`);
		} finally {
			setRestarting(false);
		}
	}

	async function handleModelSave(request: SetModelRequest) {
		await updateModel(request);
		setShowModelDialog(false);
	}

	async function handlePolicySave(policy: ToolPolicy) {
		await updatePolicy(policy);
		setPolicyEditing(false);
	}

	async function handleAddDir(path: string) {
		await orchestratorClient.addDir(agentId, path);
		await refetch();
	}

	async function handleRemoveDir(path: string) {
		await orchestratorClient.removeDir(agentId, path);
		await refetch();
	}

	async function handleClearContext() {
		try {
			const response = await clearContext();
			setConfirmClearContext(false);
			toast.success("Context cleared", {
				message: `New session #${response.new_session_number} started`,
			});
		} catch (err) {
			toast.error("Failed to clear context", {
				message:
					err instanceof Error ? err.message : "An unknown error occurred",
			});
		}
	}

	function copyId() {
		if (!agentId) return;
		navigator.clipboard.writeText(agentId).then(() => {
			setCopied(true);
			setTimeout(() => setCopied(false), 1500);
		});
	}

	// ---------------------------------------------------------------------------
	// Loading / error state
	// ---------------------------------------------------------------------------

	if (loading) {
		return (
			<div className="space-y-4">
				<CardSkeleton />
				<CardSkeleton />
			</div>
		);
	}

	if (error || !agent) {
		return (
			<div className="space-y-4">
				<button
					type="button"
					onClick={() => navigate("/agents")}
					className="flex items-center gap-1.5 text-sm text-th-text-muted hover:text-th-text-secondary"
				>
					<ArrowLeft size={14} />
					Back to agents
				</button>
				<div
					role="alert"
					className="rounded-md bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text"
				>
					{error ?? "Agent not found"}
				</div>
			</div>
		);
	}

	const isRunning = agent.status === "running";
	const canSendMessage = isRunning && !agent.config.interactive;

	const formattedCreated = new Date(agent.created_at).toLocaleString();
	const formattedUpdated = new Date(agent.updated_at).toLocaleString();

	// ---------------------------------------------------------------------------
	// Render
	// ---------------------------------------------------------------------------

	return (
		<div className="flex flex-col gap-5">
			{/* Back link */}
			<button
				type="button"
				onClick={() => navigate("/agents")}
				className="flex items-center gap-1.5 self-start text-sm text-th-text-muted hover:text-th-text-secondary"
			>
				<ArrowLeft size={14} aria-hidden="true" />
				Back to agents
			</button>

			{/* ── Agent header ────────────────────────────────────────────────── */}
			<div className="rounded-lg border border-th-border bg-th-surface p-5">
				<div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
					{/* Left: identity */}
					<div className="flex flex-col gap-2">
						<div className="flex items-center gap-3">
							<h1 className="text-xl font-semibold text-th-text">
								{agent.name}
							</h1>
							<AgentStatusBadge status={agent.status} />
						</div>

						{/* ID */}
						<div className="flex items-center gap-1.5">
							<span className="font-mono text-xs text-th-text-faint">
								{agentId}
							</span>
							<button
								type="button"
								aria-label="Copy agent ID"
								onClick={copyId}
								className="rounded p-0.5 text-th-text-muted hover:text-th-text-secondary"
							>
								<Copy size={12} />
							</button>
							{copied && (
								<span className="text-xs text-th-status-success-text">
									Copied!
								</span>
							)}
						</div>

						{/* Timestamps & model */}
						<div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-th-text-muted">
							<span>Created: {formattedCreated}</span>
							<span>Updated: {formattedUpdated}</span>
							{agent.config.model && <span>Model: {agent.config.model}</span>}
						</div>
					</div>

					{/* Right: actions — Refresh (standalone) + Actions dropdown */}
					<div className="flex flex-shrink-0 items-center gap-2">
						{/* Refresh — kept standalone: frequent, low-risk, no confirmation */}
						<button
							type="button"
							aria-label="Refresh agent data"
							onClick={refetch}
							className="rounded-md border border-th-border-strong bg-th-surface p-2 text-th-text-muted hover:bg-th-surface-hover hover:text-th-text-secondary focus:outline-none focus:ring-2 focus:ring-th-focus-ring focus:ring-offset-1"
						>
							<RefreshCw size={14} aria-hidden="true" />
						</button>

						{/* Actions dropdown */}
						<ActionsDropdown
							isRunning={isRunning}
							isFailed={agent.status === "failed" || agent.status === "stopped"}
							clearing={clearing}
							restarting={restarting}
							onChangeModel={() => setShowModelDialog(true)}
							onAddDir={() => setShowAddDirDialog(true)}
							onClearContext={() => setConfirmClearContext(true)}
							onRestart={handleRestart}
							onTerminate={() => setConfirmTerminate(true)}
						/>
					</div>
				</div>
			</div>

			{/* ── Main content + sidebar ───────────────────────────────────────── */}
			<div className="grid grid-cols-1 gap-5 lg:grid-cols-3">
				{/* Log / Terminal view (takes 2/3 width on large screens) */}
				<div className="flex flex-col gap-3 lg:col-span-2">
					{/* Tab switcher — Terminal tab only shown when backend supports PTY streaming */}
					<div
						role="tablist"
						aria-label="Agent output view"
						className="flex gap-1 rounded-lg border border-th-border bg-th-surface-sunken p-1"
					>
						<button
							role="tab"
							type="button"
							aria-selected={activeTab === "logs"}
							aria-controls="tab-panel-logs"
							id="tab-logs"
							onClick={() => setActiveTab("logs")}
							className={[
								"flex-1 rounded-md px-4 py-1.5 text-sm font-medium transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-primary-500",
								activeTab === "logs"
									? "bg-th-surface text-th-text shadow-sm"
									: "text-th-text-muted hover:text-th-text-secondary",
							].join(" ")}
						>
							Logs
						</button>
						{ptyAvailable && (
							<button
								role="tab"
								type="button"
								aria-selected={activeTab === "terminal"}
								aria-controls="tab-panel-terminal"
								id="tab-terminal"
								onClick={() => setActiveTab("terminal")}
								className={[
									"flex-1 rounded-md px-4 py-1.5 text-sm font-medium transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-primary-500",
									activeTab === "terminal"
										? "bg-th-surface text-th-text shadow-sm"
										: "text-th-text-muted hover:text-th-text-secondary",
								].join(" ")}
							>
								Terminal
							</button>
						)}
					</div>

					{/* Tab panels */}
					<div className="h-[480px]">
						<div
							role="tabpanel"
							id="tab-panel-logs"
							aria-labelledby="tab-logs"
							hidden={activeTab !== "logs"}
							className="h-full"
						>
							<AgentLogView
								lines={lines}
								status={streamStatus}
								onClear={clearLog}
							/>
						</div>
						{ptyAvailable && (
							<div
								role="tabpanel"
								id="tab-panel-terminal"
								aria-labelledby="tab-terminal"
								hidden={activeTab !== "terminal"}
								className="h-full"
							>
								{/* Mount AgentTerminal only when the Terminal tab is active to
                    avoid an unnecessary WebSocket connection while viewing Logs */}
								{activeTab === "terminal" && (
									<AgentTerminal
										agentId={agentId}
										agentInteractive={agent.config.interactive}
										readOnly={true}
									/>
								)}
							</div>
						)}
					</div>

					{/* Command input */}
					<AgentCommandInput
						agentId={agentId}
						enabled={canSendMessage}
						disabledReason={
							!isRunning
								? "Agent is not running"
								: agent.config.interactive
									? "Interactive agents do not accept commands here"
									: undefined
						}
						onSend={sendMessage}
					/>

					{/* Config panel */}
					<AgentConfigPanel
						agent={agent}
						onAddDir={handleAddDir}
						onRemoveDir={handleRemoveDir}
					/>
				</div>

				{/* Sidebar (1/3 width on large screens) */}
				<div className="flex flex-col gap-5">
					{/* Usage panel — shown only when usage data is available */}
					{usage && (
						<AgentUsagePanel
							usage={usage}
							autoClearThreshold={agent.config.auto_clear_threshold}
						/>
					)}

					{/* Todos — shown only after the agent has written at least one todo list */}
					<AgentTodosPanel agentId={agentId} />

					{/* Tool policy */}
					<section
						aria-label="Tool policy"
						className="rounded-lg border border-th-border bg-th-surface"
					>
						<div className="flex items-center justify-between border-b border-th-border px-4 py-3">
							<h2 className="text-sm font-medium text-th-text">Tool Policy</h2>
							{!policyEditing && (
								<button
									type="button"
									onClick={() => setPolicyEditing(true)}
									className="text-xs text-th-text-link hover:opacity-80"
								>
									Edit
								</button>
							)}
						</div>
						<div className="p-4">
							{policyEditing ? (
								<AgentPolicyEditor
									policy={agent.config.tool_policy}
									onSave={handlePolicySave}
								/>
							) : (
								<PolicyDisplay policy={agent.config.tool_policy} />
							)}
						</div>
					</section>

					{/* Pending approvals */}
					<div className="rounded-lg border border-th-border bg-th-surface p-4">
						<AgentApprovals
							approvals={approvals}
							loading={approvalsLoading}
							error={approvalsError}
							onApprove={approveRequest}
							onDeny={denyRequest}
						/>
					</div>
				</div>
			</div>

			{/* Dialogs */}
			<ConfirmDialog
				open={confirmTerminate}
				title="Terminate agent?"
				description={`This will permanently terminate "${agent.name}" and all associated resources. This action cannot be undone.`}
				confirmLabel="Terminate"
				variant="danger"
				loading={terminating}
				onConfirm={handleTerminate}
				onCancel={() => setConfirmTerminate(false)}
			/>

			<ClearContextDialog
				open={confirmClearContext}
				session={usage?.current_session}
				loading={clearing}
				onConfirm={handleClearContext}
				onCancel={() => setConfirmClearContext(false)}
			/>

			<ChangeModelDialog
				open={showModelDialog}
				currentModel={agent.config.model}
				onSave={handleModelSave}
				onClose={() => setShowModelDialog(false)}
			/>

			<AddDirDialog
				open={showAddDirDialog}
				onConfirm={handleAddDir}
				onClose={() => setShowAddDirDialog(false)}
			/>
		</div>
	);
}

export default AgentDetail;
