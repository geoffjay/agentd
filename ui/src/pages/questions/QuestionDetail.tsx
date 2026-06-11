/**
 * QuestionDetail — dedicated detail view for a single question.
 *
 * Route: /questions/:id
 *
 * Shows:
 * - Full question text, context, category, priority
 * - Agent, workflow, dispatch IDs
 * - All timestamps: asked_at, answered_at, expires_at
 * - Status badge
 * - Submitted answer (for Answered questions)
 * - Answer / Dismiss actions for Pending questions
 * - Back button to /questions
 */

import {
	AlertTriangle,
	ArrowLeft,
	CheckCircle,
	Clock,
	HelpCircle,
	RefreshCw,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { StatusBadge } from "@/components/common/StatusBadge";
import { AnswerDialog } from "@/components/questions/AnswerDialog";
import { askClient } from "@/services/ask";
import type { Question } from "@/types/ask";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatDateTime(iso: string): string {
	try {
		return new Date(iso).toLocaleString(undefined, {
			year: "numeric",
			month: "short",
			day: "numeric",
			hour: "2-digit",
			minute: "2-digit",
		});
	} catch {
		return iso;
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
// QuestionDetail
// ---------------------------------------------------------------------------

export function QuestionDetail() {
	const { id } = useParams<{ id: string }>();
	const navigate = useNavigate();

	const [question, setQuestion] = useState<Question | null>(null);
	const [loading, setLoading] = useState(true);
	const [error, setError] = useState<string | undefined>();

	const [answerDialogOpen, setAnswerDialogOpen] = useState(false);
	const [answering, setAnswering] = useState(false);
	const [answerError, setAnswerError] = useState<string | undefined>();
	const [answerSuccess, setAnswerSuccess] = useState(false);

	// -------------------------------------------------------------------------
	// Fetch
	// -------------------------------------------------------------------------

	const fetchQuestion = useCallback(async () => {
		if (!id) return;
		setLoading(true);
		setError(undefined);
		try {
			const q = await askClient.getQuestion(id);
			setQuestion(q);
		} catch (err) {
			setError(err instanceof Error ? err.message : "Failed to load question");
		} finally {
			setLoading(false);
		}
	}, [id]);

	useEffect(() => {
		void fetchQuestion();
	}, [fetchQuestion]);

	// -------------------------------------------------------------------------
	// Actions
	// -------------------------------------------------------------------------

	const handleSubmitAnswer = async (questionId: string, answer: string) => {
		setAnswering(true);
		setAnswerError(undefined);
		try {
			await askClient.answerQuestion(questionId, { answer });
			setAnswerSuccess(true);
			setAnswerDialogOpen(false);
			// Refresh the question to show updated status
			setTimeout(() => {
				setAnswerSuccess(false);
				void fetchQuestion();
			}, 1200);
		} catch (err) {
			setAnswerError(
				err instanceof Error ? err.message : "Failed to submit answer",
			);
		} finally {
			setAnswering(false);
		}
	};

	const handleDismiss = async () => {
		if (!question) return;
		setAnswering(true);
		setAnswerError(undefined);
		try {
			await askClient.dismissQuestion(question.id);
			void fetchQuestion();
		} catch (err) {
			setAnswerError(
				err instanceof Error ? err.message : "Failed to dismiss question",
			);
		} finally {
			setAnswering(false);
		}
	};

	// -------------------------------------------------------------------------
	// Render
	// -------------------------------------------------------------------------

	const isPending = question?.status === "Pending";
	const priorityClass = question
		? (PRIORITY_CLASSES[question.priority] ?? PRIORITY_CLASSES.Normal)
		: "";

	return (
		<div className="space-y-6">
			{/* Back navigation */}
			<div>
				<Link
					to="/questions"
					className="inline-flex items-center gap-1.5 text-sm text-th-text-muted hover:text-th-text transition-colors"
				>
					<ArrowLeft size={14} />
					Back to Questions
				</Link>
			</div>

			{/* Loading */}
			{loading && (
				<div className="space-y-4">
					<div className="h-8 w-48 rounded-md bg-th-surface animate-pulse" />
					<div className="h-40 rounded-lg border border-th-border bg-th-surface animate-pulse" />
				</div>
			)}

			{/* Error */}
			{!loading && error && (
				<div className="flex items-center gap-2 rounded-md border border-th-status-error-border bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text">
					<AlertTriangle size={14} className="flex-shrink-0" />
					<span>{error}</span>
					<button
						type="button"
						onClick={() => void fetchQuestion()}
						className="ml-auto text-xs underline hover:no-underline"
					>
						Retry
					</button>
				</div>
			)}

			{/* Action error */}
			{answerError && (
				<div className="flex items-center gap-2 rounded-md border border-th-status-error-border bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text">
					<AlertTriangle size={14} className="flex-shrink-0" />
					<span>{answerError}</span>
				</div>
			)}

			{/* Answer success */}
			{answerSuccess && (
				<div className="rounded-md border border-th-status-success-border bg-th-status-success-bg px-4 py-3 text-sm text-th-status-success-text">
					Answer submitted successfully.
				</div>
			)}

			{/* Detail card */}
			{!loading && question && (
				<div className="rounded-xl border bg-th-surface border-th-border p-6 space-y-5">
					{/* Header */}
					<div className="flex items-start justify-between gap-4">
						<div className="flex items-center gap-3">
							<div className="flex h-10 w-10 items-center justify-center rounded-lg bg-th-status-info-bg flex-shrink-0">
								<HelpCircle size={20} className="text-th-status-info-text" />
							</div>
							<div>
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
								<p className="mt-1 text-sm text-th-text-faint">
									Question Detail
								</p>
							</div>
						</div>
						<div className="flex items-center gap-3">
							<StatusBadge status={question.status} />
							<button
								type="button"
								onClick={() => void fetchQuestion()}
								disabled={loading}
								aria-label="Refresh question"
								className="text-th-text-muted hover:text-th-text disabled:opacity-50 transition-colors"
							>
								<RefreshCw
									size={14}
									className={loading ? "animate-spin" : ""}
								/>
							</button>
						</div>
					</div>

					{/* Question text */}
					<div>
						<h1 className="text-xl font-semibold text-th-text">
							{question.question}
						</h1>
					</div>

					{/* Context */}
					{question.context && (
						<div className="rounded-md bg-th-surface-sunken border border-th-border px-4 py-3">
							<p className="text-xs font-medium text-th-text-faint mb-1.5">
								Context
							</p>
							<p className="text-sm text-th-text-secondary whitespace-pre-wrap">
								{question.context}
							</p>
						</div>
					)}

					{/* Metadata grid */}
					<div className="grid grid-cols-1 sm:grid-cols-2 gap-3 text-sm">
						{/* Agent */}
						<div className="rounded-md bg-th-surface-sunken border border-th-border px-3 py-2.5">
							<p className="text-xs text-th-text-faint mb-0.5">Agent</p>
							<p className="font-mono text-xs font-medium text-th-text-secondary truncate">
								{question.agent_id}
							</p>
						</div>

						{/* Asked at */}
						<div className="rounded-md bg-th-surface-sunken border border-th-border px-3 py-2.5">
							<p className="text-xs text-th-text-faint mb-0.5 flex items-center gap-1">
								<Clock size={10} /> Asked at
							</p>
							<p className="text-xs font-medium text-th-text-secondary">
								{formatDateTime(question.asked_at)}
							</p>
						</div>

						{/* Workflow ID (optional) */}
						{question.workflow_id && (
							<div className="rounded-md bg-th-surface-sunken border border-th-border px-3 py-2.5">
								<p className="text-xs text-th-text-faint mb-0.5">Workflow</p>
								<p className="font-mono text-xs font-medium text-th-text-secondary truncate">
									{question.workflow_id}
								</p>
							</div>
						)}

						{/* Dispatch ID (optional) */}
						{question.dispatch_id && (
							<div className="rounded-md bg-th-surface-sunken border border-th-border px-3 py-2.5">
								<p className="text-xs text-th-text-faint mb-0.5">Dispatch</p>
								<p className="font-mono text-xs font-medium text-th-text-secondary truncate">
									{question.dispatch_id}
								</p>
							</div>
						)}

						{/* Answered at */}
						{question.answered_at && (
							<div className="rounded-md bg-th-surface-sunken border border-th-border px-3 py-2.5">
								<p className="text-xs text-th-text-faint mb-0.5 flex items-center gap-1">
									<Clock size={10} /> Answered at
								</p>
								<p className="text-xs font-medium text-th-text-secondary">
									{formatDateTime(question.answered_at)}
								</p>
							</div>
						)}

						{/* Expires at */}
						{question.expires_at && (
							<div className="rounded-md bg-th-surface-sunken border border-th-border px-3 py-2.5">
								<p className="text-xs text-th-text-faint mb-0.5 flex items-center gap-1">
									<Clock size={10} /> Expires at
								</p>
								<p className="text-xs font-medium text-th-text-secondary">
									{formatDateTime(question.expires_at)}
								</p>
							</div>
						)}
					</div>

					{/* Submitted answer */}
					{question.answer && (
						<div className="rounded-md bg-th-status-success-bg border border-th-status-success-border px-4 py-3">
							<p className="text-xs font-medium text-th-status-success-text flex items-center gap-1.5 mb-1">
								<CheckCircle size={12} />
								Answer submitted
							</p>
							<p className="text-sm text-th-status-success-text whitespace-pre-wrap">
								{question.answer}
							</p>
						</div>
					)}

					{/* Actions for pending questions */}
					{isPending && (
						<div className="flex gap-3 pt-1">
							<button
								type="button"
								onClick={() => setAnswerDialogOpen(true)}
								disabled={answering}
								className="rounded-md border border-th-status-warning-border bg-th-status-warning-bg px-4 py-2 text-sm font-medium text-th-status-warning-text hover:opacity-80 disabled:opacity-50 transition-colors"
							>
								Answer
							</button>
							<button
								type="button"
								onClick={() => void handleDismiss()}
								disabled={answering}
								className="rounded-md border border-th-border bg-th-surface px-4 py-2 text-sm font-medium text-th-text-muted hover:bg-th-surface-hover disabled:opacity-50 transition-colors"
							>
								{answering ? "Working…" : "Dismiss"}
							</button>
						</div>
					)}

					{/* Navigate back after dismissal */}
					{!isPending && question.status !== "Answered" && (
						<div className="pt-1">
							<button
								type="button"
								onClick={() => navigate("/questions")}
								className="text-sm text-th-text-muted hover:text-th-text transition-colors"
							>
								← Back to list
							</button>
						</div>
					)}
				</div>
			)}

			{/* Answer dialog */}
			<AnswerDialog
				open={answerDialogOpen}
				question={question}
				answering={answering}
				answerError={answerError}
				onSubmit={(qId, answer) => void handleSubmitAnswer(qId, answer)}
				onClose={() => {
					setAnswerDialogOpen(false);
					setAnswerError(undefined);
				}}
			/>
		</div>
	);
}

export default QuestionDetail;
