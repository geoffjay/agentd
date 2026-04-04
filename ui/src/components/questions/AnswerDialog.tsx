/**
 * AnswerDialog — modal for submitting an answer to a pending question.
 *
 * Shows:
 * - Question context (check type, what triggered it)
 * - Text input for free-form answer
 * - Quick-answer buttons for common responses
 * - Submit via POST /answer
 * - Success/error feedback
 */

import { MessageSquare, X, Zap } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { QuestionInfo } from "@/types/ask";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface AnswerDialogProps {
	open: boolean;
	question: QuestionInfo | null;
	answering: boolean;
	answerError?: string;
	onSubmit: (questionId: string, answer: string) => void;
	onClose: () => void;
}

// ---------------------------------------------------------------------------
// Quick answers
// ---------------------------------------------------------------------------

const QUICK_ANSWERS = [
	{ label: "Yes, start sessions", value: "yes" },
	{ label: "No, ignore for now", value: "no" },
	{ label: "Acknowledged", value: "acknowledged" },
];

// ---------------------------------------------------------------------------
// AnswerDialog
// ---------------------------------------------------------------------------

export function AnswerDialog({
	open,
	question,
	answering,
	answerError,
	onSubmit,
	onClose,
}: AnswerDialogProps) {
	const [answer, setAnswer] = useState("");
	const inputRef = useRef<HTMLTextAreaElement>(null);
	const dialogRef = useRef<HTMLDivElement>(null);

	// Reset answer text when dialog opens for a new question
	useEffect(() => {
		if (open) {
			setAnswer("");
			// Focus the textarea after the dialog opens
			setTimeout(() => inputRef.current?.focus(), 50);
		}
	}, [open, question?.question_id]);

	// Close on Escape
	useEffect(() => {
		function onKeyDown(e: KeyboardEvent) {
			if (e.key === "Escape" && open) onClose();
		}
		document.addEventListener("keydown", onKeyDown);
		return () => document.removeEventListener("keydown", onKeyDown);
	}, [open, onClose]);

	if (!open || !question) return null;

	const handleSubmit = (e: React.FormEvent) => {
		e.preventDefault();
		if (!answer.trim()) return;
		onSubmit(question.question_id, answer.trim());
	};

	const handleQuickAnswer = (value: string) => {
		onSubmit(question.question_id, value);
	};

	const checkLabel =
		question.check_type === "TmuxSessions"
			? "tmux session check"
			: question.check_type;

	return (
		<>
			{/* Backdrop */}
			<div
				className="fixed inset-0 z-40 bg-th-overlay backdrop-blur-sm"
				aria-hidden="true"
				onClick={onClose}
			/>

			{/* Dialog */}
			<div
				ref={dialogRef}
				role="dialog"
				aria-modal="true"
				aria-labelledby="answer-dialog-title"
				className="fixed inset-0 z-50 flex items-center justify-center p-4"
			>
				<div className="relative w-full max-w-lg rounded-xl bg-th-surface shadow-xl border border-th-border p-6 space-y-4">
					{/* Close button */}
					<button
						type="button"
						onClick={onClose}
						className="absolute right-4 top-4 text-th-text-muted hover:text-th-text transition-colors"
						aria-label="Close answer dialog"
					>
						<X size={18} />
					</button>

					{/* Title */}
					<div className="flex items-center gap-2.5">
						<div className="flex h-9 w-9 items-center justify-center rounded-full bg-th-status-warning-bg">
							<MessageSquare
								size={17}
								className="text-th-status-warning-text"
							/>
						</div>
						<div>
							<h2
								id="answer-dialog-title"
								className="text-base font-semibold text-th-text"
							>
								Answer Question
							</h2>
							<p className="text-xs text-th-text-faint">
								From {checkLabel}
							</p>
						</div>
					</div>

					{/* Context */}
					<div className="rounded-md bg-th-surface-sunken border border-th-border p-3 space-y-1.5 text-xs">
						<div className="flex justify-between">
							<span className="text-th-text-faint">
								Check type
							</span>
							<span className="font-medium text-th-text-secondary">
								{question.check_type}
							</span>
						</div>
						<div className="flex justify-between">
							<span className="text-th-text-faint">Asked at</span>
							<span className="font-medium text-th-text-secondary">
								{new Date(question.asked_at).toLocaleString()}
							</span>
						</div>
						<div className="flex justify-between">
							<span className="text-th-text-faint">
								Notification
							</span>
							<span className="font-mono font-medium text-th-text-secondary truncate max-w-[180px]">
								{question.notification_id}
							</span>
						</div>
					</div>

					{/* Quick answers */}
					<div className="space-y-1.5">
						<p className="text-xs font-medium text-th-text-muted flex items-center gap-1">
							<Zap size={11} /> Quick answers
						</p>
						<div className="flex flex-wrap gap-2">
							{QUICK_ANSWERS.map((qa) => (
								<button
									key={qa.value}
									type="button"
									disabled={answering}
									onClick={() => handleQuickAnswer(qa.value)}
									className="rounded-full border border-th-border px-3 py-1 text-xs text-th-text-secondary hover:bg-th-surface-hover disabled:opacity-50 transition-colors"
								>
									{qa.label}
								</button>
							))}
						</div>
					</div>

					{/* Free-form answer */}
					<form onSubmit={handleSubmit} className="space-y-3">
						<div>
							<label
								htmlFor="answer-input"
								className="block text-xs font-medium text-th-text-secondary mb-1"
							>
								Custom answer
							</label>
							<textarea
								id="answer-input"
								ref={inputRef}
								rows={3}
								value={answer}
								onChange={(e) => setAnswer(e.target.value)}
								placeholder="Type your answer…"
								disabled={answering}
								className="w-full rounded-md border border-th-border-input bg-th-input px-3 py-2 text-sm text-th-text placeholder:text-th-text-faint focus:outline-none focus:ring-2 focus:ring-th-focus-ring disabled:opacity-50 resize-none"
							/>
						</div>

						{/* Error */}
						{answerError && (
							<p className="text-xs text-th-status-error-text">
								{answerError}
							</p>
						)}

						{/* Actions */}
						<div className="flex justify-end gap-2">
							<button
								type="button"
								onClick={onClose}
								disabled={answering}
								className="rounded-md border border-th-border px-4 py-2 text-sm text-th-text-muted hover:bg-th-surface-hover disabled:opacity-50 transition-colors"
							>
								Cancel
							</button>
							<button
								type="submit"
								disabled={!answer.trim() || answering}
								className="rounded-md bg-th-accent hover:bg-th-accent-hover disabled:opacity-50 px-4 py-2 text-sm font-medium text-th-accent-text transition-colors"
							>
								{answering ? "Submitting…" : "Submit Answer"}
							</button>
						</div>
					</form>
				</div>
			</div>
		</>
	);
}

export default AnswerDialog;
