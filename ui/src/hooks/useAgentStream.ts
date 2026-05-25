/**
 * useAgentStream — WebSocket hook for real-time agent log streaming.
 *
 * Connects to ws://<host>/v2/stream/<agentId>. The v2 protocol delivers a
 * deterministic snapshot-then-live ordering of every conversation event for
 * the agent in a single connection:
 *
 *   { frame: "snapshot_begin", cursor: N, agent_id: ... }
 *   { frame: "event", seq: K, type: "agent:output", ... }
 *   { frame: "event", seq: K+1, type: "agent:tool_use", ... }
 *   ...
 *   { frame: "snapshot_end", seq: N }
 *   { frame: "event", seq: N+1, ... }   // live phase
 *
 * Reconnects pass `since_seq` so only the delta replays. The last observed
 * seq is persisted to sessionStorage so a fresh tab open within the same
 * session resumes cleanly. Buffered log history itself is NOT cached
 * locally — the server snapshot is the source of truth.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { serviceConfig } from "@/services/config";
import { agentEventBus } from "@/services/eventBus";
import { WebSocketManager } from "@/services/websocket";
import type {
	AgentEvent,
	AgentThinkingEvent,
	AgentToolUseEvent,
	ContextClearedEvent,
	UsageUpdateEvent,
} from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type StreamStatus = "connecting" | "connected" | "disconnected";

export interface LogLine {
	id: number;
	/** Raw text (may contain ANSI escape sequences) */
	text: string;
	timestamp: string;
	/** When set, this line represents a tool call rather than plain output */
	toolUse?: {
		tool_name: string;
		tool_id: string;
		tool_input: Record<string, unknown>;
		summary: string;
	};
	/** When true, this line is a thinking/reasoning block */
	isThinking?: boolean;
	/** When true, this line is a reconnection gap marker (broadcast lag) */
	isSeparator?: boolean;
}

/** Callback invoked when a real-time usage update event arrives */
export type UsageUpdateCallback = (event: UsageUpdateEvent) => void;

/** Callback invoked when a context cleared event arrives */
export type ContextClearedCallback = (event: ContextClearedEvent) => void;

export interface UseAgentStreamOptions {
	/** Called when an agent:usage_update event arrives on the stream */
	onUsageUpdate?: UsageUpdateCallback;
	/** Called when an agent:context_cleared event arrives on the stream */
	onContextCleared?: ContextClearedCallback;
}

export interface UseAgentStreamResult {
	lines: LogLine[];
	status: StreamStatus;
	/** True between snapshot_begin and snapshot_end — UIs may show a spinner. */
	historyLoading: boolean;
	/** Clear all buffered log lines (does not disconnect the stream) */
	clear: () => void;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_LINES = 5_000;

// ---------------------------------------------------------------------------
// sessionStorage — last-seq resume cursor only
// ---------------------------------------------------------------------------

const SEQ_STORAGE_KEY = (agentId: string) => `agentd:last-seq:${agentId}`;

function loadLastSeq(agentId: string): number {
	try {
		const raw = sessionStorage.getItem(SEQ_STORAGE_KEY(agentId));
		if (!raw) return 0;
		const n = Number.parseInt(raw, 10);
		return Number.isFinite(n) && n > 0 ? n : 0;
	} catch {
		return 0;
	}
}

function saveLastSeq(agentId: string, seq: number): void {
	try {
		sessionStorage.setItem(SEQ_STORAGE_KEY(agentId), String(seq));
	} catch {
		// sessionStorage unavailable — silently ignore
	}
}

function clearLastSeq(agentId: string): void {
	try {
		sessionStorage.removeItem(SEQ_STORAGE_KEY(agentId));
	} catch {
		// ignore
	}
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let globalLineId = 0;

function makeLogLine(text: string, timestamp: string): LogLine {
	return {
		id: ++globalLineId,
		text,
		timestamp,
	};
}

function makeToolUseLine(event: AgentToolUseEvent): LogLine {
	return {
		id: ++globalLineId,
		text: `[${event.tool_name}] ${event.summary}`,
		timestamp: event.timestamp,
		toolUse: {
			tool_name: event.tool_name,
			tool_id: event.tool_id,
			tool_input: event.tool_input,
			summary: event.summary,
		},
	};
}

function makeThinkingLine(text: string, timestamp: string): LogLine {
	return {
		id: ++globalLineId,
		text,
		timestamp,
		isThinking: true,
	};
}

function makeGapLine(skipped: number, timestamp: string): LogLine {
	return {
		id: ++globalLineId,
		text: `─── Stream gap · ${skipped} events missed (broadcast lag) ───`,
		timestamp,
		isSeparator: true,
	};
}

function capLines(prev: LogLine[], incoming: LogLine[]): LogLine[] {
	const combined = [...prev, ...incoming];
	if (combined.length <= MAX_LINES) return combined;
	return combined.slice(combined.length - MAX_LINES);
}

function agentStreamUrl(agentId: string): string {
	const wsBase = serviceConfig.orchestratorServiceUrl.replace(/^http/, "ws");
	return `${wsBase}/v2/stream/${agentId}`;
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

interface V2Frame {
	frame?: string;
	seq?: number;
	type?: string;
	[key: string]: unknown;
}

export function useAgentStream(
	agentId: string,
	options: UseAgentStreamOptions = {},
): UseAgentStreamResult {
	const [lines, setLines] = useState<LogLine[]>([]);
	const [status, setStatus] = useState<StreamStatus>("connecting");
	const [historyLoading, setHistoryLoading] = useState(false);

	const linesRef = useRef<LogLine[]>([]);
	const lastSeqRef = useRef<number>(0);

	// Store callbacks in refs so the WebSocket effect doesn't re-run when
	// callbacks change.
	const onUsageUpdateRef = useRef(options.onUsageUpdate);
	const onContextClearedRef = useRef(options.onContextCleared);
	onUsageUpdateRef.current = options.onUsageUpdate;
	onContextClearedRef.current = options.onContextCleared;

	const managerRef = useRef<WebSocketManager | null>(null);

	useEffect(() => {
		// Resume from the last seq we recorded for this agent in this tab. The
		// server replays only events with seq > since_seq, so the new
		// connection lands on the same canonical event ordering the previous
		// session saw.
		lastSeqRef.current = loadLastSeq(agentId);
		linesRef.current = [];
		setLines([]);

		const manager = new WebSocketManager(agentStreamUrl(agentId), {
			heartbeatInterval: 0,
		});
		managerRef.current = manager;

		const sendSubscribe = () => {
			manager.send(
				JSON.stringify({
					frame: "subscribe",
					since_seq: lastSeqRef.current,
				}),
			);
		};

		const unsubState = manager.onStateChange((state) => {
			switch (state) {
				case "Connected":
					setStatus("connected");
					// (Re)send the subscribe frame with the latest seq cursor.
					sendSubscribe();
					break;
				case "Disconnected":
					setStatus("disconnected");
					break;
				default:
					setStatus("connecting");
			}
		});

		const handleEventFrame = (frame: V2Frame) => {
			if (typeof frame.seq === "number" && frame.seq > lastSeqRef.current) {
				lastSeqRef.current = frame.seq;
				saveLastSeq(agentId, frame.seq);
			}

			// The frame is a v2 envelope plus the v1 event payload. The
			// inner shape matches AgentEvent (the live broadcast shape), so
			// we can re-emit it through the bus and dispatch on `type` as
			// before.
			const parsed = frame as unknown as AgentEvent;
			agentEventBus.emit(parsed);

			if (parsed.type === "agent:usage_update") {
				onUsageUpdateRef.current?.(parsed);
				return;
			}
			if (parsed.type === "agent:context_cleared") {
				onContextClearedRef.current?.(parsed);
				return;
			}

			let line: LogLine | null = null;
			if (parsed.type === "agent:output") {
				line = makeLogLine(parsed.line, parsed.timestamp);
			} else if (parsed.type === "agent:tool_use") {
				line = makeToolUseLine(parsed);
			} else if (parsed.type === "agent:thinking") {
				const thinking = parsed as AgentThinkingEvent;
				line = makeThinkingLine(thinking.text, thinking.timestamp);
			}

			if (line) {
				const newLine = line;
				setLines((prev) => {
					const next = capLines(prev, [newLine]);
					linesRef.current = next;
					return next;
				});
			}
		};

		const unsubMsg = manager.onMessage((event: MessageEvent) => {
			let frame: V2Frame | null = null;
			try {
				frame = JSON.parse(String(event.data)) as V2Frame;
			} catch {
				return;
			}

			switch (frame.frame) {
				case "snapshot_begin":
					setHistoryLoading(true);
					return;
				case "snapshot_end":
					setHistoryLoading(false);
					if (typeof frame.seq === "number") {
						lastSeqRef.current = Math.max(lastSeqRef.current, frame.seq);
						saveLastSeq(agentId, lastSeqRef.current);
					}
					return;
				case "event":
					handleEventFrame(frame);
					return;
				case "gap": {
					const skipped =
						typeof frame.skipped === "number" ? frame.skipped : 0;
					const gap = makeGapLine(skipped, new Date().toISOString());
					setLines((prev) => {
						const next = capLines(prev, [gap]);
						linesRef.current = next;
						return next;
					});
					return;
				}
				case "error":
					// Surface as a separator-style line so the user sees it
					// without spamming console-only logs.
					setLines((prev) => {
						const msg =
							typeof frame?.message === "string"
								? frame.message
								: "unknown stream error";
						const errLine: LogLine = {
							id: ++globalLineId,
							text: `─── Stream error · ${msg} ───`,
							timestamp: new Date().toISOString(),
							isSeparator: true,
						};
						const next = capLines(prev, [errLine]);
						linesRef.current = next;
						return next;
					});
					return;
				default:
					// Unknown frame — ignore.
					return;
			}
		});

		manager.connect();

		return () => {
			unsubState();
			unsubMsg();
			manager.disconnect();
			managerRef.current = null;
		};
	}, [agentId]);

	const clear = useCallback(() => {
		linesRef.current = [];
		setLines([]);
		lastSeqRef.current = 0;
		clearLastSeq(agentId);
	}, [agentId]);

	return { lines, status, historyLoading, clear };
}
