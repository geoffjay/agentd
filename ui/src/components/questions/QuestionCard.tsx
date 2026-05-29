/**
 * QuestionCard — displays a single question with status and action controls.
 *
 * Shows:
 * - Question text, category, priority badge
 * - Agent ID and asked timestamp
 * - Collapsible context block
 * - Status badge: Pending (yellow) / Answered (green) / Dismissed (gray) / Expired (gray)
 * - Answer and Dismiss buttons for Pending questions
 * - Submitted answer for Answered questions
 */

import {
	ChevronDown,
	ChevronRight,
	Clock,
	ExternalLink,
	User,
} from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import { StatusBadge } from "@/components/common/StatusBadge";
import type { Question } from "@/types/ask";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface QuestionCardProps {
	question: Question;
	onAnswer: (question: Question) => void;
	onDismiss?: (question: Question) => void;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatAsked(isoString: string): string {
	try {
		return new Date(isoString).toLocaleString();
	} catch {
		return isoString;
	}
}

const PRIORITY_CLASSES: Record<string, string> = {
	Urgent:
		"bg-th-status-error-bg text-th-status-error-text border border-th-status-error-border",
	High: "bg-th-status-warning-bg text-th-status-warning-text border border-th-status-warning-border",
	Normal:
		"bg-th-status-info-bg text-th-status-info-text border border-th-status-info-border",
	Low: "bg-th-surface-sunken text-th-text-muted border border-th-border",
};

// ---------------------------------------------------------------------------
// QuestionCard
// ---------------------------------------------------------------------------

export function QuestionCard({
	question,
	onAnswer,
	onDismiss,
}: QuestionCardProps) {
	const isPending = question.status === "Pending";
	const [contextExpanded, setContextExpanded] = useState(false);

	const priorityClass =
		PRIORITY_CLASSES[question.priority] ?? PRIORITY_CLASSES.Normal;

	return (
		<div
			className={[
				"rounded-lg border bg-th-surface p-4 space-y-3",
				isPending ? "border-th-status-warning-border" : "border-th-border",
			].join(" ")}
		>
			{/* Header: category + priority + status */}
			<div className="flex items-start justify-between gap-2">
				<div className="flex-1 min-w-0">
					<div className="flex items-center gap-2 flex-wrap">
						<span className="text-xs font-medium text-th-text-secondary uppercase tracking-wide">
							{question.category}
						</span>
						<span
							className={[
								"rounded-full px-2 py-0.5 text-xs font-medium",
								priorityClass,
							].join(" ")}
						>
							{question.priority}
						</span>
					</div>
					<p className="mt-1 text-sm font-medium text-th-text line-clamp-2">
						{question.question}
					</p>
					<Link
						to={`/questions/${question.id}`}
						className="mt-1 inline-flex items-center gap-1 text-xs text-th-text-link hover:underline"
					>
						<ExternalLink size={10} />
						View detail
					</Link>
				</div>
				<StatusBadge status={question.status} />
			</div>

			{/* Metadata: agent + timestamp */}
			<div className="flex items-center gap-3 text-xs text-th-text-faint flex-wrap">
				<span className="flex items-center gap-1">
					<User size={11} />
					<span className="font-mono">{question.agent_id}</span>
				</span>
				<span className="flex items-center gap-1">
					<Clock size={11} />
					{formatAsked(question.asked_at)}
				</span>
			</div>

			{/* Collapsible context */}
			{question.context && (
				<div>
					<button
						type="button"
						onClick={() => setContextExpanded((e) => !e)}
						className="flex items-center gap-1 text-xs text-th-text-faint hover:text-th-text-secondary transition-colors"
						aria-expanded={contextExpanded}
					>
						{contextExpanded ? (
							<ChevronDown size={12} />
						) : (
							<ChevronRight size={12} />
						)}
						{contextExpanded ? "Hide" : "Show"} context
					</button>
					{contextExpanded && (
						<p className="mt-1.5 text-xs text-th-text-secondary bg-th-surface-sunken rounded-md border border-th-border px-3 py-2 whitespace-pre-wrap">
							{question.context}
						</p>
					)}
				</div>
			)}

			{/* Submitted answer (if answered) */}
			{question.answer && (
				<div className="rounded-md bg-th-status-success-bg border border-th-status-success-border px-3 py-2">
					<p className="text-xs font-medium text-th-status-success-text">
						Answer submitted
					</p>
					<p className="mt-0.5 text-xs text-th-status-success-text">
						{question.answer}
					</p>
				</div>
			)}

			{/* Action buttons for pending questions */}
			{isPending && (
				<div className="flex gap-2">
					<button
						type="button"
						onClick={() => onAnswer(question)}
						className="flex-1 rounded-md border border-th-status-warning-border bg-th-status-warning-bg px-3 py-1.5 text-xs font-medium text-th-status-warning-text hover:opacity-80 transition-colors"
					>
						Answer
					</button>
					{onDismiss && (
						<button
							type="button"
							onClick={() => onDismiss(question)}
							className="rounded-md border border-th-border bg-th-surface px-3 py-1.5 text-xs font-medium text-th-text-muted hover:bg-th-surface-hover transition-colors"
						>
							Dismiss
						</button>
					)}
				</div>
			)}
		</div>
	);
}

export default QuestionCard;
