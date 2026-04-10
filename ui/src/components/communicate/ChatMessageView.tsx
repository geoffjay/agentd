/**
 * ChatMessageView — displays message history for a room.
 *
 * Features:
 * - Loads and renders message history
 * - Auto-scrolls to the latest message on new arrivals
 * - Infinite scroll upward (load older messages)
 * - Visual distinction between agent and human messages
 * - Thread reply indicator for messages with reply_to
 * - Loading states for initial load and older-page load
 */

import { Bot, CornerUpLeft, Loader2, User } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef } from "react";
import type { ChatMessage, ParticipantKind } from "@/types/communicate";

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
}

export function ChatMessageView({
	messages,
	loading,
	loadingOlder,
	hasMore,
	onLoadOlder,
	roomId,
}: ChatMessageViewProps) {
	const bottomRef = useRef<HTMLDivElement>(null);
	const containerRef = useRef<HTMLDivElement>(null);
	const prevScrollHeightRef = useRef<number>(0);
	const initialScrollDone = useRef(false);

	// Reset the initial-scroll flag whenever the active room changes so that
	// switching rooms always brings the user to the bottom of the new room.
	useEffect(() => {
		initialScrollDone.current = false;
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

	// Auto-scroll to bottom when new messages arrive
	useEffect(() => {
		const container = containerRef.current;
		if (!container) return;

		// If the user is near the bottom, keep them there
		const distanceFromBottom =
			container.scrollHeight - container.scrollTop - container.clientHeight;

		if (distanceFromBottom < 120) {
			bottomRef.current?.scrollIntoView({ behavior: "smooth" });
		}
	}, [messages]);

	// Maintain scroll position when older messages are prepended
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

	// Infinite scroll: fire when user scrolls near the top
	const handleScroll = useCallback(() => {
		const container = containerRef.current;
		if (!container || loadingOlder || !hasMore) return;
		if (container.scrollTop < 80) {
			onLoadOlder();
		}
	}, [loadingOlder, hasMore, onLoadOlder]);

	// Build lookup map for reply references — memoised to avoid re-allocating on every render
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
		<div
			ref={containerRef}
			onScroll={handleScroll}
			className="flex-1 overflow-y-auto px-4 py-4 space-y-4"
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

			<div ref={bottomRef} />
		</div>
	);
}
