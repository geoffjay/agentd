/**
 * ChatMessageView — displays message history for a room.
 *
 * Features:
 * - Loads and renders message history
 * - Auto-scrolls to the latest message on new arrivals
 * - Scroll-lock: pauses auto-scroll when user scrolls up; "Jump to latest" button resumes
 * - New-message counter on the "Jump to latest" button while scroll-locked
 * - Infinite scroll upward (load older messages)
 * - Visual distinction between agent and human messages
 * - Thread reply indicator for messages with reply_to
 * - Loading states for initial load and older-page load
 * - Thinking indicator: animated dots with agent name(s) when agent(s) are busy
 */

import { ArrowDown, Bot, CornerUpLeft, Loader2, User } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ChatMessage, Participant, ParticipantKind } from "@/types/communicate";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatTime(iso: string): string {
	const date = new Date(iso);
	if (isNaN(date.getTime())) return "";
	return new Intl.DateTimeFormat(undefined, {
		hour: "2-digit",
		minute: "2-digit",
	}).format(date);
}

// ---------------------------------------------------------------------------
// Sender avatar
// ---------------------------------------------------------------------------

function SenderAvatar({ kind }: { kind: ParticipantKind }) {
	return (
		<div
			className={[
				"flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-th-accent-text",
				kind === "agent" ? "bg-th-accent" : "bg-th-status-success-dot",
			].join(" ")}
			aria-hidden="true"
		>
			{kind === "agent" ? <Bot size={16} /> : <User size={16} />}
		</div>
	);
}

// ---------------------------------------------------------------------------
// Kind badge
// ---------------------------------------------------------------------------

function KindBadge({ kind }: { kind: ParticipantKind }) {
	return (
		<span
			className={[
				"rounded px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide",
				kind === "agent"
					? "bg-th-accent/10 text-th-text-link"
					: "bg-th-status-success-bg text-th-status-success-text",
			].join(" ")}
		>
			{kind}
		</span>
	);
}

// ---------------------------------------------------------------------------
// Single message bubble
// ---------------------------------------------------------------------------

interface MessageBubbleProps {
	message: ChatMessage;
	replyToMessage?: ChatMessage;
}

function MessageBubble({ message, replyToMessage }: MessageBubbleProps) {
	return (
		<div className="flex items-start gap-3 group">
			<SenderAvatar kind={message.sender_kind} />

			<div className="min-w-0 flex-1">
				{/* Header */}
				<div className="flex items-center gap-2 mb-1">
					<span className="text-sm font-semibold text-th-text">
						{message.sender_name}
					</span>
					<KindBadge kind={message.sender_kind} />
					<span className="text-xs text-th-text-muted">
						{formatTime(message.created_at)}
					</span>
				</div>

				{/* Reply indicator */}
				{replyToMessage && (
					<div className="mb-1 flex items-center gap-1.5 rounded-md border-l-2 border-th-border-strong bg-th-surface-raised/50 px-2 py-1 text-xs text-th-text-muted">
						<CornerUpLeft size={12} className="shrink-0" />
						<span className="font-medium">{replyToMessage.sender_name}</span>
						<span className="truncate">{replyToMessage.content}</span>
					</div>
				)}

				{/* Content */}
				<p className="whitespace-pre-wrap break-words text-sm text-th-text leading-relaxed">
					{message.content}
				</p>
			</div>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Thinking indicator
// ---------------------------------------------------------------------------

function ThinkingIndicator({ agents }: { agents: Participant[] }) {
	if (agents.length === 0) return null;

	let label: string;
	if (agents.length === 1) {
		label = `${agents[0].display_name} is thinking…`;
	} else if (agents.length === 2) {
		label = `${agents[0].display_name} and ${agents[1].display_name} are thinking…`;
	} else {
		label = `${agents.length} agents are thinking…`;
	}

	return (
		<div
			className="flex items-center gap-2 px-1 text-sm text-th-text-muted"
			aria-live="polite"
			aria-label={label}
		>
			{/* Three bouncing dots */}
			<span className="flex items-center gap-0.5" aria-hidden="true">
				<span className="h-1.5 w-1.5 rounded-full bg-th-text-muted animate-bounce [animation-delay:-0.3s]" />
				<span className="h-1.5 w-1.5 rounded-full bg-th-text-muted animate-bounce [animation-delay:-0.15s]" />
				<span className="h-1.5 w-1.5 rounded-full bg-th-text-muted animate-bounce" />
			</span>
			<span>{label}</span>
		</div>
	);
}

// ---------------------------------------------------------------------------
// ChatMessageView
// ---------------------------------------------------------------------------

interface ChatMessageViewProps {
	messages: ChatMessage[];
	loading: boolean;
	loadingOlder: boolean;
	hasMore: boolean;
	onLoadOlder: () => void;
	/** Room identifier — used to reset scroll position when switching rooms. */
	roomId?: string;
	/** Agent participants currently processing (activity_state === "busy"). */
	busyAgents?: Participant[];
}

export function ChatMessageView({
	messages,
	loading,
	loadingOlder,
	hasMore,
	onLoadOlder,
	roomId,
	busyAgents = [],
}: ChatMessageViewProps) {
	const bottomRef = useRef<HTMLDivElement>(null);
	const containerRef = useRef<HTMLDivElement>(null);
	const prevScrollHeightRef = useRef<number>(0);
	const initialScrollDone = useRef(false);

	// Scroll-lock: true when the user has scrolled away from the bottom.
	const [scrollLocked, setScrollLocked] = useState(false);

	// Count of new messages that arrived while scroll-locked.
	const [newMessageCount, setNewMessageCount] = useState(0);
	// Ref used to diff messages.length across renders without stale closures.
	const prevMessageCountRef = useRef(0);
	// Detects the loadingOlder true→false transition so prepended historical
	// messages don't get counted as "new".
	const prevLoadingOlderRef = useRef(false);

	// Reset all scroll state when the active room changes.
	useEffect(() => {
		initialScrollDone.current = false;
		setScrollLocked(false);
		setNewMessageCount(0);
		prevMessageCountRef.current = 0;
		prevLoadingOlderRef.current = false;
	}, [roomId]);

	// Scroll to the bottom on initial load or room switch.  The flag prevents
	// this from re-triggering when the user loads older messages (infinite
	// scroll upward), because by then initialScrollDone is already true.
	useEffect(() => {
		if (messages.length > 0 && !loading && !initialScrollDone.current) {
			bottomRef.current?.scrollIntoView({ behavior: "instant" });
			initialScrollDone.current = true;
		}
	}, [messages, loading]);

	// Auto-scroll to bottom when new messages arrive (only when not scroll-locked).
	useEffect(() => {
		if (scrollLocked) return;
		const container = containerRef.current;
		if (!container) return;

		const distanceFromBottom =
			container.scrollHeight - container.scrollTop - container.clientHeight;

		if (distanceFromBottom < 120) {
			bottomRef.current?.scrollIntoView({ behavior: "smooth" });
		}
	}, [messages, scrollLocked]);

	// Count new messages that arrive while scroll-locked, without counting
	// historical messages prepended by infinite scroll.
	useEffect(() => {
		const wasLoadingOlder = prevLoadingOlderRef.current;
		prevLoadingOlderRef.current = loadingOlder;

		if (loadingOlder) return; // still fetching older page

		const currentCount = messages.length;

		if (wasLoadingOlder) {
			// Older messages just finished loading — absorb the prepended count
			// without incrementing the new-message indicator.
			prevMessageCountRef.current = currentCount;
			return;
		}

		const delta = currentCount - prevMessageCountRef.current;
		if (scrollLocked && delta > 0 && !loading) {
			setNewMessageCount((c) => c + delta);
		}
		prevMessageCountRef.current = currentCount;
	}, [messages.length, loading, loadingOlder, scrollLocked]);

	// Maintain scroll position when older messages are prepended.
	useEffect(() => {
		const container = containerRef.current;
		if (!container || !loadingOlder) return;
		prevScrollHeightRef.current = container.scrollHeight;
	}, [loadingOlder]);

	useEffect(() => {
		const container = containerRef.current;
		if (!container || loadingOlder || prevScrollHeightRef.current === 0) return;
		const delta = container.scrollHeight - prevScrollHeightRef.current;
		if (delta > 0) {
			container.scrollTop += delta;
		}
		prevScrollHeightRef.current = 0;
	}, [messages, loadingOlder]);

	// Jump to the bottom and release the scroll lock.
	const resumeScroll = useCallback(() => {
		setScrollLocked(false);
		setNewMessageCount(0);
		const container = containerRef.current;
		if (container) {
			container.scrollTop = container.scrollHeight;
		}
	}, []);

	// Infinite scroll upward + scroll-lock detection.
	const handleScroll = useCallback(() => {
		const container = containerRef.current;
		if (!container) return;

		// Detect whether the user has scrolled away from the bottom.
		const atBottom =
			container.scrollHeight - container.scrollTop - container.clientHeight < 120;
		setScrollLocked(!atBottom);

		// Trigger infinite scroll when near the top.
		if (!loadingOlder && hasMore && container.scrollTop < 80) {
			onLoadOlder();
		}
	}, [loadingOlder, hasMore, onLoadOlder]);

	// Build lookup map for reply references — memoised to avoid re-allocating on every render.
	const messageMap = useMemo(
		() => new Map(messages.map((m) => [m.id, m])),
		[messages],
	);

	if (loading) {
		return (
			<div className="flex flex-1 items-center justify-center">
				<Loader2 size={24} className="animate-spin text-th-text-muted" />
			</div>
		);
	}

	if (messages.length === 0) {
		return (
			<div className="flex flex-1 items-center justify-center">
				<p className="text-sm text-th-text-muted">
					No messages yet. Start the conversation!
				</p>
			</div>
		);
	}

	return (
		<div className="relative flex-1 overflow-hidden">
			{/* Scrollable message list */}
			<div
				ref={containerRef}
				onScroll={handleScroll}
				className="h-full overflow-y-auto px-4 py-4 space-y-4"
				aria-label="Chat messages"
				aria-live="polite"
				aria-relevant="additions"
			>
				{/* Load older indicator */}
				{loadingOlder && (
					<div className="flex justify-center py-2">
						<Loader2 size={16} className="animate-spin text-th-text-muted" />
					</div>
				)}
				{!hasMore && messages.length > 0 && (
					<p className="text-center text-xs text-th-text-muted py-1">
						Beginning of conversation
					</p>
				)}

				{messages.map((msg) => (
					<MessageBubble
						key={msg.id}
						message={msg}
						replyToMessage={
							msg.reply_to ? messageMap.get(msg.reply_to) : undefined
						}
					/>
				))}

				{/* Thinking indicator — shown when one or more agents are busy */}
				<ThinkingIndicator agents={busyAgents} />

				<div ref={bottomRef} />
			</div>

			{/* Jump-to-latest button — appears when scroll-locked */}
			{scrollLocked && (
				<div className="pointer-events-none absolute inset-x-0 bottom-4 flex justify-center">
					<button
						type="button"
						onClick={resumeScroll}
						aria-label={
							newMessageCount > 0
								? `${newMessageCount} new message${newMessageCount === 1 ? "" : "s"} — jump to latest`
								: "Jump to latest messages"
						}
						className="pointer-events-auto flex items-center gap-1.5 rounded-full bg-th-accent px-4 py-2 text-sm font-medium text-th-accent-text shadow-lg transition-colors hover:bg-th-accent/90"
					>
						<ArrowDown size={14} aria-hidden="true" />
						{newMessageCount > 0
							? `${newMessageCount} new message${newMessageCount === 1 ? "" : "s"}`
							: "Jump to latest"}
					</button>
				</div>
			)}
		</div>
	);
}
