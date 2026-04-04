/**
 * QuestionCard — displays a single question with status and answer controls.
 *
 * Shows:
 * - Check type, asked timestamp, notification ID (linked)
 * - Status badge: Pending (yellow) / Answered (green) / Expired (gray)
 * - "Answer" button for Pending questions
 * - Submitted answer for Answered questions
 */

import { Clock, Link } from "lucide-react";
import { StatusBadge } from "@/components/common/StatusBadge";
import type { QuestionInfo } from "@/types/ask";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface QuestionCardProps {
	question: QuestionInfo;
	onAnswer: (question: QuestionInfo) => void;
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

const CHECK_TYPE_LABELS: Record<string, string> = {
	TmuxSessions: "Tmux Sessions",
};

// ---------------------------------------------------------------------------
// QuestionCard
// ---------------------------------------------------------------------------

export function QuestionCard({ question, onAnswer }: QuestionCardProps) {
	const isPending = question.status === "Pending";

	return (
		<div
			className={[
				"rounded-lg border bg-th-surface p-4 space-y-3",
				isPending
					? "border-th-status-warning-border"
					: "border-th-border",
			].join(" ")}
		>
			{/* Header: check type + status */}
			<div className="flex items-start justify-between gap-2">
				<div>
					<p className="text-sm font-medium text-th-text">
						{CHECK_TYPE_LABELS[question.check_type] ?? question.check_type}
					</p>
					<p className="mt-0.5 flex items-center gap-1 text-xs text-th-text-faint">
						<Clock size={11} />
						{formatAsked(question.asked_at)}
					</p>
				</div>
				<StatusBadge status={question.status} />
			</div>

			{/* Notification ID */}
			<div className="flex items-center gap-1.5">
				<Link size={11} className="text-th-text-muted flex-shrink-0" />
				<span className="text-xs text-th-text-faint">
					Notification
				</span>
				<span className="font-mono text-xs text-th-text-secondary truncate">
					{question.notification_id}
				</span>
			</div>

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

			{/* Answer button for pending questions */}
			{isPending && (
				<button
					type="button"
					onClick={() => onAnswer(question)}
					className="w-full rounded-md border border-th-status-warning-border bg-th-status-warning-bg px-3 py-1.5 text-xs font-medium text-th-status-warning-text hover:opacity-80 transition-colors"
				>
					Answer
				</button>
			)}
		</div>
	);
}

export default QuestionCard;
