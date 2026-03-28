/**
 * AgentTerminal — xterm.js terminal connected to the orchestrator PTY stream relay.
 *
 * Features:
 * - Full ANSI color/cursor support via xterm.js
 * - Binary WebSocket frames for PTY output; JSON text frames for resize/control
 * - Auto-resize to container via FitAddon + ResizeObserver
 * - Mode-aware input routing:
 *   - Interactive-mode agents (agentInteractive=true): read-only / interactive
 *     toggle forwards keystrokes directly to PTY stdin as binary frames.
 *   - SDK-mode agents (agentInteractive=false, default): terminal is always
 *     read-only; a compact compose input in the toolbar sends messages via
 *     POST /agents/{id}/message so they reach Claude over the SDK WebSocket.
 * - Exponential backoff reconnection (matches WebSocketManager behaviour)
 * - Graceful fallback when the backend does not support PTY streaming
 * - Web links addon (clickable URLs in output)
 * - Search addon with inline search bar
 * - Theme matches the existing UI dark/light palette
 */

import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { Terminal } from "@xterm/xterm";
import { useCallback, useEffect, useRef, useState } from "react";
import "@xterm/xterm/css/xterm.css";
import {
	ChevronDown,
	ChevronUp,
	Info,
	Keyboard,
	KeyboardOff,
	Loader2,
	Search,
	Send,
	TerminalSquare,
	Wifi,
	WifiOff,
	X,
} from "lucide-react";
import { serviceConfig } from "@/services/config";
import { orchestratorClient } from "@/services/orchestrator";

// ---------------------------------------------------------------------------
// Terminal WebSocket URL
// ---------------------------------------------------------------------------

function agentTerminalUrl(agentId: string): string {
	const wsBase = serviceConfig.orchestratorServiceUrl.replace(/^http/, "ws");
	return `${wsBase}/terminal/${agentId}`;
}

// ---------------------------------------------------------------------------
// Connection state
// ---------------------------------------------------------------------------

type TerminalStatus =
	| "connecting"
	| "connected"
	| "disconnected"
	| "unavailable";

// After this many consecutive failures without a successful connection, stop
// retrying and show the "PTY not available" fallback UI.
const MAX_CONSECUTIVE_FAILURES = 3;
const MIN_RECONNECT_DELAY = 1_000;
const MAX_RECONNECT_DELAY = 30_000;

// ---------------------------------------------------------------------------
// xterm.js dark theme — matches the existing gray-950 log view palette
// ---------------------------------------------------------------------------

const TERMINAL_THEME = {
	background: "#030712", // Tailwind gray-950
	foreground: "#e5e7eb", // gray-200
	cursor: "#e5e7eb",
	cursorAccent: "#030712",
	selectionBackground: "#374151", // gray-700
	selectionForeground: "#f9fafb", // gray-50
	// Standard 16 ANSI colours
	black: "#1f2937",
	red: "#f87171",
	green: "#4ade80",
	yellow: "#fbbf24",
	blue: "#60a5fa",
	magenta: "#c084fc",
	cyan: "#22d3ee",
	white: "#e5e7eb",
	brightBlack: "#374151",
	brightRed: "#fca5a5",
	brightGreen: "#86efac",
	brightYellow: "#fde68a",
	brightBlue: "#93c5fd",
	brightMagenta: "#d8b4fe",
	brightCyan: "#67e8f9",
	brightWhite: "#f9fafb",
};

// ---------------------------------------------------------------------------
// Status badge
// ---------------------------------------------------------------------------

function TerminalStatusBadge({ status }: { status: TerminalStatus }) {
	if (status === "connected") {
		return (
			<span
				aria-label="Terminal connected"
				className="flex items-center gap-1 text-xs text-green-500 dark:text-green-400"
			>
				<Wifi size={12} aria-hidden="true" />
				Connected
			</span>
		);
	}
	if (status === "connecting") {
		return (
			<span
				aria-label="Terminal connecting"
				className="flex items-center gap-1 text-xs text-yellow-500 dark:text-yellow-400"
			>
				<Loader2 size={12} aria-hidden="true" className="animate-spin" />
				Connecting…
			</span>
		);
	}
	if (status === "unavailable") {
		return (
			<span
				aria-label="PTY not available"
				className="flex items-center gap-1 text-xs text-gray-500 dark:text-gray-500"
			>
				<TerminalSquare size={12} aria-hidden="true" />
				PTY unavailable
			</span>
		);
	}
	return (
		<span
			aria-label="Terminal disconnected"
			className="flex items-center gap-1 text-xs text-red-500 dark:text-red-400"
		>
			<WifiOff size={12} aria-hidden="true" />
			Disconnected
		</span>
	);
}

// ---------------------------------------------------------------------------
// PTY mode badge
// ---------------------------------------------------------------------------

function TerminalModeBadge({ interactive }: { interactive: boolean }) {
	return (
		<span
			aria-label={interactive ? "PTY interactive mode" : "PTY SDK mode"}
			className="flex items-center gap-1 rounded bg-gray-800 px-1.5 py-0.5 font-mono text-xs text-gray-400"
		>
			PTY · {interactive ? "Interactive" : "SDK"}
		</span>
	);
}

// ---------------------------------------------------------------------------
// SDK-mode info banner
// ---------------------------------------------------------------------------

function SdkModeBanner({ onDismiss }: { onDismiss: () => void }) {
	return (
		<div
			role="note"
			aria-label="SDK mode info"
			className="flex items-center gap-2 border-b border-blue-800 bg-blue-950/50 px-3 py-1.5 text-xs text-blue-300"
		>
			<Info size={12} aria-hidden="true" className="shrink-0" />
			<span className="flex-1">
				SDK mode — This terminal shows raw protocol output. Use the{" "}
				<strong className="text-blue-200">Logs</strong> tab for structured
				output.
			</span>
			<button
				type="button"
				aria-label="Dismiss SDK mode info"
				onClick={onDismiss}
				className="rounded p-0.5 text-blue-400 hover:bg-blue-800 hover:text-white"
			>
				<X size={12} aria-hidden="true" />
			</button>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Unavailable fallback
// ---------------------------------------------------------------------------

function UnavailableFallback({ onRetry }: { onRetry: () => void }) {
	return (
		<div className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center">
			<TerminalSquare size={32} className="text-gray-600" aria-hidden="true" />
			<div>
				<p className="text-sm font-medium text-gray-400">
					PTY streaming not available
				</p>
				<p className="mt-1 text-xs text-gray-600">
					This agent&apos;s backend does not support PTY streaming. Terminal
					output is only available with a PTY-backed session (e.g. the wrap
					service in PTY mode). Agents running on tmux or Docker backends use
					the <strong className="text-gray-400">Logs</strong> tab instead.
				</p>
			</div>
			<button
				type="button"
				onClick={onRetry}
				className="rounded-md border border-gray-600 px-3 py-1.5 text-xs text-gray-300 hover:bg-gray-800"
			>
				Retry connection
			</button>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Component props
// ---------------------------------------------------------------------------

export interface AgentTerminalProps {
	agentId: string;
	/**
	 * Whether the agent was launched in interactive mode (`config.interactive=true`,
	 * no `--sdk-url`). Controls how keyboard input is routed:
	 *
	 * - `true`  — PTY stdin path: keystrokes are forwarded as binary WebSocket
	 *             frames to `/terminal/{agentId}`. An Interactive/Read-only toggle
	 *             in the toolbar lets the user enable or disable input forwarding.
	 * - `false` (default, SDK mode) — the terminal is always read-only. A compact
	 *             compose input in the toolbar sends messages via
	 *             `POST /agents/{agentId}/message` so they reach Claude over the
	 *             SDK WebSocket protocol.
	 */
	agentInteractive?: boolean;
	/**
	 * When `agentInteractive=true`: controls whether the Interactive toggle starts
	 * in the on position (false) or off position (true, default).
	 * When `agentInteractive=false` (SDK mode): ignored — xterm is always read-only.
	 */
	readOnly?: boolean;
}

// ---------------------------------------------------------------------------
// AgentTerminal
// ---------------------------------------------------------------------------

export function AgentTerminal({
	agentId,
	agentInteractive = false,
	readOnly = true,
}: AgentTerminalProps) {
	const containerRef = useRef<HTMLDivElement>(null);

	// xterm.js refs
	const termRef = useRef<Terminal | null>(null);
	const fitAddonRef = useRef<FitAddon | null>(null);
	const searchAddonRef = useRef<SearchAddon | null>(null);

	// WebSocket refs
	const wsRef = useRef<WebSocket | null>(null);
	const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
	const reconnectDelayRef = useRef(MIN_RECONNECT_DELAY);
	const consecutiveFailuresRef = useRef(0);
	const intentionalCloseRef = useRef(false);

	// Stable ref for agentInteractive so the onData closure can read it.
	// In practice this won't change after mount, but the ref keeps the closure
	// honest without re-running the init effect.
	const agentInteractiveRef = useRef(agentInteractive);
	useEffect(() => {
		agentInteractiveRef.current = agentInteractive;
	}, [agentInteractive]);

	// UI state — interactive toggle (only meaningful when agentInteractive=true)
	const [interactive, setInteractive] = useState<boolean>(!readOnly);
	const [status, setStatus] = useState<TerminalStatus>("connecting");
	const [searchOpen, setSearchOpen] = useState(false);
	const [searchTerm, setSearchTerm] = useState("");

	// SDK-mode compose state (only used when agentInteractive=false)
	const [sdkMessage, setSdkMessage] = useState("");
	const [sdkSending, setSdkSending] = useState(false);
	const [sdkError, setSdkError] = useState<string | undefined>();
	const [showSdkBanner, setShowSdkBanner] = useState(true);

	// Keep a ref in sync with interactive state so the onData closure can read it
	const interactiveRef = useRef(!readOnly);
	useEffect(() => {
		interactiveRef.current = interactive;
		// Only sync xterm stdin and focus for interactive-mode agents.
		// In SDK mode xterm is permanently read-only; the compose input handles input.
		if (agentInteractive && termRef.current) {
			termRef.current.options.disableStdin = !interactive;
			// Auto-focus the terminal when switching to interactive mode so the user
			// can type immediately without having to click inside the terminal canvas.
			if (interactive) {
				termRef.current.focus();
			}
		}
	}, [interactive, agentInteractive]);

	// ---------------------------------------------------------------------------
	// SDK-mode: send message via POST /agents/{id}/message
	// ---------------------------------------------------------------------------

	const handleSdkSend = useCallback(async () => {
		const msg = sdkMessage.trim();
		if (!msg || sdkSending || status !== "connected") return;
		setSdkSending(true);
		setSdkError(undefined);
		try {
			await orchestratorClient.sendMessage(agentId, msg);
			setSdkMessage("");
		} catch (err) {
			setSdkError(
				err instanceof Error ? err.message : "Failed to send message",
			);
		} finally {
			setSdkSending(false);
		}
	}, [agentId, sdkMessage, sdkSending, status]);

	function handleSdkKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
		if (e.key === "Enter" && !e.shiftKey) {
			e.preventDefault();
			void handleSdkSend();
		}
	}

	// ---------------------------------------------------------------------------
	// Initialise xterm.js terminal (mount only)
	// ---------------------------------------------------------------------------

	useEffect(() => {
		const el = containerRef.current;
		if (!el) return;

		const term = new Terminal({
			theme: TERMINAL_THEME,
			fontFamily:
				'"JetBrains Mono", "Fira Code", Consolas, "Courier New", monospace',
			fontSize: 13,
			lineHeight: 1.4,
			cursorBlink: true,
			cursorStyle: "block",
			scrollback: 5_000,
			// SDK-mode agents: terminal is always read-only (input goes via compose box).
			// Interactive-mode agents: follow the readOnly prop.
			disableStdin: agentInteractive ? readOnly : true,
			// Allow the terminal to receive focus for copy/paste
			allowProposedApi: false,
		});

		const fitAddon = new FitAddon();
		const webLinksAddon = new WebLinksAddon();
		const searchAddon = new SearchAddon();

		term.loadAddon(fitAddon);
		term.loadAddon(webLinksAddon);
		term.loadAddon(searchAddon);

		term.open(el);
		// Defer initial fit — the element may not have its final size yet
		requestAnimationFrame(() => {
			fitAddon.fit();
		});

		termRef.current = term;
		fitAddonRef.current = fitAddon;
		searchAddonRef.current = searchAddon;

		// Forward keyboard input → PTY stdin (interactive-mode agents only).
		// SDK-mode agents have disableStdin=true so onData never fires, but we
		// guard on agentInteractiveRef anyway as a belt-and-suspenders check.
		const disposeOnData = term.onData((data) => {
			if (!agentInteractiveRef.current) return;
			if (
				interactiveRef.current &&
				wsRef.current?.readyState === WebSocket.OPEN
			) {
				wsRef.current.send(new TextEncoder().encode(data));
			}
		});

		return () => {
			disposeOnData.dispose();
			term.dispose();
			termRef.current = null;
			fitAddonRef.current = null;
			searchAddonRef.current = null;
		};
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []); // Run once on mount — terminal is independent of agentId changes

	// ---------------------------------------------------------------------------
	// WebSocket connection with exponential backoff
	// ---------------------------------------------------------------------------

	const connect = useCallback(() => {
		if (intentionalCloseRef.current) return;
		// Already connecting or connected — skip
		if (
			wsRef.current &&
			(wsRef.current.readyState === WebSocket.CONNECTING ||
				wsRef.current.readyState === WebSocket.OPEN)
		) {
			return;
		}

		setStatus("connecting");

		const ws = new WebSocket(agentTerminalUrl(agentId));
		ws.binaryType = "arraybuffer";
		wsRef.current = ws;

		ws.onopen = () => {
			reconnectDelayRef.current = MIN_RECONNECT_DELAY;
			consecutiveFailuresRef.current = 0;
			setStatus("connected");

			// Send the current terminal dimensions so the server can size the PTY
			if (termRef.current) {
				ws.send(
					JSON.stringify({
						type: "resize",
						cols: termRef.current.cols,
						rows: termRef.current.rows,
					}),
				);
			}
		};

		ws.onmessage = (event: MessageEvent) => {
			const term = termRef.current;
			if (!term) return;
			if (event.data instanceof ArrayBuffer) {
				// Binary frame — raw PTY output bytes
				term.write(new Uint8Array(event.data));
			} else {
				// Text frame — typically a JSON error from the server before close
				const text = String(event.data);
				try {
					const msg = JSON.parse(text) as { error?: string };
					if (msg.error) {
						term.writeln(`\r\n\x1b[31m[agentd] ${msg.error}\x1b[0m`);
						return;
					}
				} catch {
					// Not JSON — write as-is
				}
				term.write(text);
			}
		};

		// onerror is always followed by onclose; let onclose handle reconnect
		ws.onerror = () => {};

		ws.onclose = () => {
			wsRef.current = null;
			if (intentionalCloseRef.current) {
				setStatus("disconnected");
				return;
			}

			consecutiveFailuresRef.current++;

			// After MAX_CONSECUTIVE_FAILURES without a successful open, stop retrying
			// and show the unavailable fallback (likely a non-PTY backend returning 404).
			if (consecutiveFailuresRef.current >= MAX_CONSECUTIVE_FAILURES) {
				setStatus("unavailable");
				return;
			}

			setStatus("connecting");
			reconnectTimerRef.current = setTimeout(() => {
				reconnectTimerRef.current = null;
				connect();
			}, reconnectDelayRef.current);
			reconnectDelayRef.current = Math.min(
				reconnectDelayRef.current * 2,
				MAX_RECONNECT_DELAY,
			);
		};
	}, [agentId]);

	// Start connection when component mounts; clean up on unmount
	useEffect(() => {
		intentionalCloseRef.current = false;
		reconnectDelayRef.current = MIN_RECONNECT_DELAY;
		consecutiveFailuresRef.current = 0;

		connect();

		return () => {
			intentionalCloseRef.current = true;
			if (reconnectTimerRef.current !== null) {
				clearTimeout(reconnectTimerRef.current);
				reconnectTimerRef.current = null;
			}
			const ws = wsRef.current;
			if (ws) {
				ws.onopen = null;
				ws.onclose = null;
				ws.onerror = null;
				ws.onmessage = null;
				ws.close();
				wsRef.current = null;
			}
		};
	}, [agentId, connect]);

	// ---------------------------------------------------------------------------
	// ResizeObserver — fit terminal when container dimensions change
	// ---------------------------------------------------------------------------

	useEffect(() => {
		const el = containerRef.current;
		if (!el) return;

		let rafId: number | null = null;

		const observer = new ResizeObserver(() => {
			// Throttle via rAF to avoid excessive fit/resize calls during drag
			if (rafId !== null) return;
			rafId = requestAnimationFrame(() => {
				rafId = null;
				const fit = fitAddonRef.current;
				const term = termRef.current;
				if (!fit || !term) return;
				try {
					fit.fit();
				} catch {
					// FitAddon can throw if the element has zero size
					return;
				}
				if (wsRef.current?.readyState === WebSocket.OPEN) {
					wsRef.current.send(
						JSON.stringify({
							type: "resize",
							cols: term.cols,
							rows: term.rows,
						}),
					);
				}
			});
		});

		observer.observe(el);
		return () => {
			observer.disconnect();
			if (rafId !== null) cancelAnimationFrame(rafId);
		};
	}, []);

	// ---------------------------------------------------------------------------
	// Search helpers
	// ---------------------------------------------------------------------------

	const handleSearchNext = useCallback(() => {
		if (!searchAddonRef.current || !searchTerm.trim()) return;
		searchAddonRef.current.findNext(searchTerm, { incremental: false });
	}, [searchTerm]);

	const handleSearchPrev = useCallback(() => {
		if (!searchAddonRef.current || !searchTerm.trim()) return;
		searchAddonRef.current.findPrevious(searchTerm, { incremental: false });
	}, [searchTerm]);

	function handleSearchKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
		if (e.key === "Enter") {
			e.shiftKey ? handleSearchPrev() : handleSearchNext();
		}
		if (e.key === "Escape") {
			setSearchOpen(false);
		}
	}

	// ---------------------------------------------------------------------------
	// Retry (reset failure count and reconnect)
	// ---------------------------------------------------------------------------

	const handleRetry = useCallback(() => {
		consecutiveFailuresRef.current = 0;
		reconnectDelayRef.current = MIN_RECONNECT_DELAY;
		connect();
	}, [connect]);

	// ---------------------------------------------------------------------------
	// Toolbar button styles (match AgentLogView)
	// ---------------------------------------------------------------------------

	const toolbarBtnCls =
		"flex items-center gap-1 rounded px-2 py-0.5 text-xs text-gray-400 hover:bg-gray-700 hover:text-white";
	const toolbarBtnActiveCls =
		"flex items-center gap-1 rounded px-2 py-0.5 text-xs text-blue-400 hover:bg-gray-700";

	// ---------------------------------------------------------------------------
	// Render
	// ---------------------------------------------------------------------------

	return (
		<div
			aria-label="Agent terminal output"
			className="flex h-full flex-col overflow-hidden rounded-lg border border-gray-700 bg-gray-950"
		>
			{/* Toolbar */}
			<div className="flex items-center justify-between border-b border-gray-700 bg-gray-900 px-3 py-2">
				<div className="flex items-center gap-2">
					<TerminalStatusBadge status={status} />
					<TerminalModeBadge interactive={agentInteractive} />
				</div>

				<div className="flex items-center gap-2">
					{/* Search toggle */}
					<button
						type="button"
						aria-label={searchOpen ? "Close search" : "Search terminal output"}
						onClick={() => setSearchOpen((v) => !v)}
						className={searchOpen ? toolbarBtnActiveCls : toolbarBtnCls}
					>
						<Search size={12} aria-hidden="true" />
						Search
					</button>

					{agentInteractive ? (
						/* Interactive-mode: PTY stdin toggle */
						<button
							type="button"
							aria-label={
								interactive
									? "Switch to read-only mode"
									: "Switch to interactive mode"
							}
							onClick={() => setInteractive((v) => !v)}
							title={
								interactive
									? "Interactive: keyboard input is forwarded to the PTY"
									: "Read-only: keyboard input is not forwarded"
							}
							className={interactive ? toolbarBtnActiveCls : toolbarBtnCls}
						>
							{interactive ? (
								<Keyboard size={12} aria-hidden="true" />
							) : (
								<KeyboardOff size={12} aria-hidden="true" />
							)}
							{interactive ? "Interactive" : "Read-only"}
						</button>
					) : (
						/* SDK-mode: compact compose input — sends via POST /agents/{id}/message */
						<div className="flex items-center gap-1">
							<input
								type="text"
								aria-label="Send message to agent"
								value={sdkMessage}
								onChange={(e) => setSdkMessage(e.target.value)}
								onKeyDown={handleSdkKeyDown}
								placeholder="Message agent…"
								disabled={sdkSending || status !== "connected"}
								className="w-48 rounded border border-gray-600 bg-gray-800 px-2 py-0.5 font-mono text-xs text-gray-200 placeholder-gray-600 focus:border-gray-400 focus:outline-none disabled:opacity-50"
							/>
							<button
								type="button"
								aria-label="Send message to agent"
								onClick={() => void handleSdkSend()}
								disabled={
									!sdkMessage.trim() || sdkSending || status !== "connected"
								}
								className="rounded p-1 text-gray-400 hover:bg-gray-700 hover:text-white disabled:opacity-40"
								title="Send message (Enter)"
							>
								{sdkSending ? (
									<Loader2
										size={12}
										aria-hidden="true"
										className="animate-spin"
									/>
								) : (
									<Send size={12} aria-hidden="true" />
								)}
							</button>
						</div>
					)}
				</div>
			</div>

			{/* SDK-mode info banner — dismissible, shown by default for SDK agents */}
			{!agentInteractive && showSdkBanner && (
				<SdkModeBanner onDismiss={() => setShowSdkBanner(false)} />
			)}

			{/* SDK-mode error banner */}
			{!agentInteractive && sdkError && (
				<div
					role="alert"
					className="border-b border-red-800 bg-red-950 px-3 py-1 text-xs text-red-400"
				>
					{sdkError}
				</div>
			)}

			{/* Search bar */}
			{searchOpen && (
				<div className="flex items-center gap-2 border-b border-gray-700 bg-gray-900 px-3 py-1.5">
					<input
						type="text"
						aria-label="Search terminal output"
						value={searchTerm}
						onChange={(e) => setSearchTerm(e.target.value)}
						onKeyDown={handleSearchKeyDown}
						placeholder="Search… (Enter next, Shift+Enter prev)"
						className="flex-1 rounded border border-gray-600 bg-gray-800 px-2 py-1 font-mono text-xs text-gray-200 placeholder-gray-600 focus:border-gray-400 focus:outline-none"
						// eslint-disable-next-line jsx-a11y/no-autofocus
						autoFocus
					/>
					<button
						type="button"
						aria-label="Previous match"
						onClick={handleSearchPrev}
						disabled={!searchTerm.trim()}
						className="rounded p-1 text-gray-400 hover:bg-gray-700 hover:text-white disabled:opacity-40"
					>
						<ChevronUp size={12} aria-hidden="true" />
					</button>
					<button
						type="button"
						aria-label="Next match"
						onClick={handleSearchNext}
						disabled={!searchTerm.trim()}
						className="rounded p-1 text-gray-400 hover:bg-gray-700 hover:text-white disabled:opacity-40"
					>
						<ChevronDown size={12} aria-hidden="true" />
					</button>
					<button
						type="button"
						aria-label="Dismiss search"
						onClick={() => setSearchOpen(false)}
						className="rounded p-1 text-gray-400 hover:bg-gray-700 hover:text-white"
					>
						<X size={12} aria-hidden="true" />
					</button>
				</div>
			)}

			{/* Terminal container — always mounted so xterm stays attached to the DOM.
          Hidden via the HTML `hidden` attribute when the PTY is unavailable so
          that term.open() does not need to re-run after a Retry click. */}
			<div
				ref={containerRef}
				hidden={status === "unavailable"}
				className="flex-1 overflow-hidden"
				style={{
					// Slight inset padding keeps the xterm canvas away from the border
					padding: "4px 8px",
					// Prevent xterm from overflowing its container
					minWidth: 0,
					minHeight: 0,
				}}
			/>

			{/* Unavailable fallback — rendered as a sibling when PTY is unavailable */}
			{status === "unavailable" && (
				<UnavailableFallback onRetry={handleRetry} />
			)}
		</div>
	);
}

export default AgentTerminal;
