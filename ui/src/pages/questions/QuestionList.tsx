/**
 * QuestionList — main questions page using the v0.12.0 Q&A model.
 *
 * Features:
 * - Service health indicator
 * - Filter bar: status (All/Pending/Answered/Dismissed/Expired), category
 * - Questions grid with answer and dismiss actions
 * - Pagination
 * - Auto-refresh polling with configurable interval
 * - AnswerDialog modal
 */

import {
	AlertTriangle,
	HelpCircle,
	RefreshCw,
	ToggleLeft,
	ToggleRight,
	Wifi,
	WifiOff,
} from "lucide-react";
import { useState } from "react";
import { AnswerDialog } from "@/components/questions/AnswerDialog";
import { QuestionCard } from "@/components/questions/QuestionCard";
import { Pagination } from "@/components/common/Pagination";
import { useAskService, POLLING_INTERVAL_OPTIONS } from "@/hooks/useAskService";
import type { Question, QuestionStatus } from "@/types/ask";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PAGE_SIZE = 12;

const STATUS_FILTERS: Array<{ value: QuestionStatus | "All"; label: string }> =
	[
		{ value: "All", label: "All" },
		{ value: "Pending", label: "Pending" },
		{ value: "Answered", label: "Answered" },
		{ value: "Dismissed", label: "Dismissed" },
		{ value: "Expired", label: "Expired" },
	];

const POLLING_LABELS: Record<number, string> = {
	5000: "5s",
	15000: "15s",
	30000: "30s",
	60000: "1m",
};

// ---------------------------------------------------------------------------
// QuestionList
// ---------------------------------------------------------------------------

export function QuestionList() {
	const {
		health,
		recheckHealth,
		questions,
		total,
		loading,
		error,
		filters,
		setStatusFilter,
		setFilters,
		busyIds,
		answerQuestion,
		dismissQuestion,
		actionError,
		pollingEnabled,
		pollingInterval,
		setPollingEnabled,
		setPollingInterval,
		refetch,
	} = useAskService({ params: { limit: PAGE_SIZE, offset: 0 } });

	const [statusFilter, setLocalStatusFilter] = useState<QuestionStatus | "All">(
		"All",
	);
	const [categoryFilter, setCategoryFilter] = useState("");
	const [page, setPage] = useState(1);
	const [answerTarget, setAnswerTarget] = useState<Question | null>(null);
	const [answerSuccess, setAnswerSuccess] = useState(false);

	// Derive unique categories from loaded questions
	const categories = [...new Set(questions.map((q) => q.category))].sort();

	// Apply client-side status/category filter
	const filtered = questions.filter((q) => {
		if (statusFilter !== "All" && q.status !== statusFilter) return false;
		if (categoryFilter && q.category !== categoryFilter) return false;
		return true;
	});

	const pendingCount = questions.filter((q) => q.status === "Pending").length;
	const totalPages = Math.ceil(filtered.length / PAGE_SIZE);
	const paginated = filtered.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);

	// -------------------------------------------------------------------------
	// Handlers
	// -------------------------------------------------------------------------

	const handleStatusFilter = (s: QuestionStatus | "All") => {
		setLocalStatusFilter(s);
		setPage(1);
		setStatusFilter(s === "All" ? undefined : s);
	};

	const handleCategoryFilter = (cat: string) => {
		setCategoryFilter(cat);
		setPage(1);
		if (cat) {
			setFilters({ ...filters, category: cat });
		} else {
			const next = { ...filters };
			delete next.category;
			setFilters(next);
		}
	};

	const handleAnswer = (question: Question) => {
		setAnswerTarget(question);
		setAnswerSuccess(false);
	};

	const handleSubmitAnswer = async (questionId: string, answer: string) => {
		const ok = await answerQuestion(questionId, answer);
		if (ok) {
			setAnswerSuccess(true);
			setTimeout(() => {
				setAnswerTarget(null);
				setAnswerSuccess(false);
			}, 1200);
		}
	};

	const handleDismiss = async (question: Question) => {
		await dismissQuestion(question.id);
	};

	// -------------------------------------------------------------------------
	// Render
	// -------------------------------------------------------------------------

	return (
		<div className="space-y-6">
			{/* Page header */}
			<div className="flex items-start justify-between gap-4 flex-wrap">
				<div className="flex items-center gap-3">
					<div className="flex h-10 w-10 items-center justify-center rounded-lg bg-th-status-info-bg">
						<HelpCircle size={20} className="text-th-status-info-text" />
					</div>
					<div>
						<h1 className="text-2xl font-semibold text-th-text">Questions</h1>
						<p className="text-sm text-th-text-muted">
							Agent questions waiting for your response.
						</p>
					</div>
				</div>

				{/* Health + polling controls */}
				<div className="flex items-center gap-4 flex-wrap">
					{/* Polling toggle */}
					<div className="flex items-center gap-2">
						<button
							type="button"
							role="switch"
							aria-checked={pollingEnabled}
							onClick={() => setPollingEnabled(!pollingEnabled)}
							className="flex items-center gap-1.5 text-xs text-th-text-secondary hover:text-th-text transition-colors"
							title={pollingEnabled ? "Auto-refresh on" : "Auto-refresh off"}
						>
							{pollingEnabled ? (
								<>
									<ToggleRight size={18} className="text-th-text-link" />
									<RefreshCw size={11} className="animate-spin text-th-text-link" />
								</>
							) : (
								<ToggleLeft size={18} className="text-th-text-muted" />
							)}
							<span className="hidden sm:inline">Auto-refresh</span>
						</button>

						{pollingEnabled && (
							<div
								className="flex items-center rounded-md border border-th-border overflow-hidden text-xs"
								role="group"
								aria-label="Polling interval"
							>
								{POLLING_INTERVAL_OPTIONS.map((ms) => (
									<button
										key={ms}
										type="button"
										onClick={() => setPollingInterval(ms)}
										aria-pressed={pollingInterval === ms}
										className={[
											"px-2 py-1 transition-colors",
											pollingInterval === ms
												? "bg-th-accent/10 text-th-text-link font-medium"
												: "text-th-text-muted hover:text-th-text",
										].join(" ")}
									>
										{POLLING_LABELS[ms]}
									</button>
								))}
							</div>
						)}
					</div>

					{/* Refresh button */}
					<button
						type="button"
						onClick={refetch}
						disabled={loading}
						aria-label="Refresh questions"
						className="flex items-center gap-1.5 text-xs text-th-text-muted hover:text-th-text disabled:opacity-50 transition-colors"
					>
						<RefreshCw size={13} className={loading ? "animate-spin" : ""} />
						<span className="hidden sm:inline">Refresh</span>
					</button>

					{/* Service health */}
					{health.checking ? (
						<div className="flex items-center gap-1.5 text-xs text-th-text-muted">
							<RefreshCw size={12} className="animate-spin" />
							Checking…
						</div>
					) : health.reachable ? (
						<div className="flex items-center gap-1.5 text-xs text-th-status-success-text">
							<Wifi size={13} />
							Ask service
							{health.version && (
								<span className="text-th-text-muted">v{health.version}</span>
							)}
						</div>
					) : (
						<button
							type="button"
							onClick={recheckHealth}
							className="flex items-center gap-1.5 text-xs text-th-status-error-text hover:underline"
						>
							<WifiOff size={13} />
							Unreachable - retry
						</button>
					)}
				</div>
			</div>

			{/* Error banner */}
			{error && (
				<div className="flex items-center gap-2 rounded-md border border-th-status-error-border bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text">
					<AlertTriangle size={14} className="flex-shrink-0" />
					<span>{error}</span>
				</div>
			)}

			{/* Action error */}
			{actionError && (
				<div className="flex items-center gap-2 rounded-md border border-th-status-error-border bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text">
					<AlertTriangle size={14} className="flex-shrink-0" />
					<span>{actionError}</span>
				</div>
			)}

			{/* Answer success */}
			{answerSuccess && (
				<div className="rounded-md border border-th-status-success-border bg-th-status-success-bg px-4 py-3 text-sm text-th-status-success-text">
					Answer submitted successfully.
				</div>
			)}

			{/* Filter bar */}
			<div className="flex items-center gap-3 flex-wrap">
				{/* Status filter */}
				<div
					className="flex items-center rounded-md border border-th-border overflow-hidden text-xs"
					role="group"
					aria-label="Filter by status"
				>
					{STATUS_FILTERS.map(({ value, label }) => (
						<button
							key={value}
							type="button"
							onClick={() => handleStatusFilter(value)}
							aria-pressed={statusFilter === value}
							className={[
								"px-3 py-1.5 transition-colors",
								statusFilter === value
									? "bg-th-accent/10 text-th-text-link font-medium"
									: "text-th-text-muted hover:text-th-text",
							].join(" ")}
						>
							{label}
							{value === "Pending" && pendingCount > 0 && (
								<span className="ml-1.5 rounded-full bg-th-status-warning-bg px-1.5 py-0.5 text-xs font-medium text-th-status-warning-text">
									{pendingCount}
								</span>
							)}
						</button>
					))}
				</div>

				{/* Category filter */}
				{categories.length > 0 && (
					<select
						value={categoryFilter}
						onChange={(e) => handleCategoryFilter(e.target.value)}
						aria-label="Filter by category"
						className="rounded-md border border-th-border bg-th-surface px-3 py-1.5 text-xs text-th-text-secondary focus:outline-none focus:ring-2 focus:ring-th-focus-ring"
					>
						<option value="">All categories</option>
						{categories.map((cat) => (
							<option key={cat} value={cat}>
								{cat}
							</option>
						))}
					</select>
				)}

				{/* Total count */}
				<span className="ml-auto text-xs text-th-text-faint">
					{filtered.length} question{filtered.length !== 1 ? "s" : ""}
					{total > filtered.length && ` (${total} total)`}
				</span>
			</div>

			{/* Questions grid */}
			{loading && questions.length === 0 ? (
				<div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
					{Array.from({ length: 3 }).map((_, i) => (
						<div
							key={i}
							className="h-40 rounded-lg border border-th-border bg-th-surface animate-pulse"
						/>
					))}
				</div>
			) : paginated.length === 0 ? (
				<div className="rounded-lg border border-dashed border-th-border bg-th-surface py-12 text-center">
					<HelpCircle size={32} className="mx-auto mb-3 text-th-text-faint" />
					<p className="text-sm text-th-text-muted">
						{statusFilter === "All" && !categoryFilter
							? "No questions yet."
							: "No questions match your filters."}
					</p>
				</div>
			) : (
				<div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
					{paginated.map((question) => (
						<QuestionCard
							key={question.id}
							question={question}
							onAnswer={handleAnswer}
							onDismiss={
								question.status === "Pending"
									? (q) => void handleDismiss(q)
									: undefined
							}
						/>
					))}
				</div>
			)}

			{/* Pagination */}
			{totalPages > 1 && (
				<Pagination
					page={page}
					totalPages={totalPages}
					totalItems={filtered.length}
					pageSize={PAGE_SIZE}
					onPageChange={setPage}
				/>
			)}

			{/* Answer dialog */}
			<AnswerDialog
				open={answerTarget !== null}
				question={answerTarget}
				answering={answerTarget !== null && busyIds.has(answerTarget.id)}
				answerError={actionError}
				onSubmit={(id, answer) => void handleSubmitAnswer(id, answer)}
				onClose={() => setAnswerTarget(null)}
			/>
		</div>
	);
}

export default QuestionList;
