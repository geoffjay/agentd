/**
 * QuestionList — main questions page assembling all ask-service components.
 *
 * Layout:
 * - Page header with service health indicator
 * - Service connection info (ask health + notify URL)
 * - Check controls (run trigger, auto-trigger toggle)
 * - Environment status (tmux card)
 * - Questions list with status filters
 * - AnswerDialog (modal)
 */

import {
	AlertTriangle,
	HelpCircle,
	RefreshCw,
	Wifi,
	WifiOff,
} from "lucide-react";
import { useState } from "react";
import { AnswerDialog } from "@/components/questions/AnswerDialog";
import { CheckControls } from "@/components/questions/CheckControls";
import { EnvironmentStatus } from "@/components/questions/EnvironmentStatus";
import { QuestionCard } from "@/components/questions/QuestionCard";
import { useAskService } from "@/hooks/useAskService";
import type { QuestionInfo, QuestionStatus } from "@/types/ask";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type StatusFilter = QuestionStatus | "All";

const STATUS_FILTERS: StatusFilter[] = [
	"All",
	"Pending",
	"Answered",
	"Expired",
];

// ---------------------------------------------------------------------------
// QuestionList
// ---------------------------------------------------------------------------

export function QuestionList() {
	const {
		health,
		recheckHealth,
		triggering,
		lastTriggerResult,
		lastTriggerAt,
		triggerError,
		runTrigger,
		autoTrigger,
		autoTriggerInterval,
		setAutoTrigger,
		setAutoTriggerInterval,
		questions,
		answering,
		answerError,
		submitAnswer,
	} = useAskService();

	const [statusFilter, setStatusFilter] = useState<StatusFilter>("All");
	const [answerTarget, setAnswerTarget] = useState<QuestionInfo | null>(null);
	const [answerSuccess, setAnswerSuccess] = useState(false);

	const filteredQuestions =
		statusFilter === "All"
			? questions
			: questions.filter((q) => q.status === statusFilter);

	const pendingCount = questions.filter((q) => q.status === "Pending").length;

	const handleAnswer = (question: QuestionInfo) => {
		setAnswerTarget(question);
		setAnswerSuccess(false);
	};

	const handleSubmitAnswer = async (questionId: string, answer: string) => {
		const ok = await submitAnswer(questionId, answer);
		if (ok) {
			setAnswerSuccess(true);
			setTimeout(() => {
				setAnswerTarget(null);
				setAnswerSuccess(false);
			}, 1200);
		}
	};

	const tmux = lastTriggerResult?.results?.tmux_sessions;

	return (
		<div className="space-y-6">
			{/* Page header */}
			<div className="flex items-start justify-between gap-4 flex-wrap">
				<div className="flex items-center gap-3">
					<div className="flex h-10 w-10 items-center justify-center rounded-lg bg-th-status-info-bg">
						<HelpCircle
							size={20}
							className="text-th-status-info-text"
						/>
					</div>
					<div>
						<h1 className="text-2xl font-semibold text-th-text">
							Questions
						</h1>
						<p className="text-sm text-th-text-muted">
							Pending questions waiting for your response.
						</p>
					</div>
				</div>

				{/* Service health indicator */}
				<div className="flex items-center gap-2">
					{health.checking ? (
						<div className="flex items-center gap-1.5 text-xs text-th-text-muted">
							<RefreshCw size={12} className="animate-spin" />
							Checking…
						</div>
					) : health.reachable ? (
						<div className="flex items-center gap-1.5 text-xs text-th-status-success-text">
							<Wifi size={13} />
							Ask service · port 17001
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
							Ask service unreachable - retry
						</button>
					)}
				</div>
			</div>

			{/* Notify service warning */}
			{health.reachable && !health.notifyUrl && (
				<div className="flex items-center gap-2 rounded-md border border-th-status-warning-border bg-th-status-warning-bg px-4 py-3 text-sm text-th-status-warning-text">
					<AlertTriangle size={14} className="flex-shrink-0" />
					<span>
						Could not determine the connected notify service URL. Answers may
						not be delivered.
					</span>
				</div>
			)}
			{health.reachable && health.notifyUrl && (
				<div className="flex items-center gap-2 text-xs text-th-text-faint">
					<span>Connected notify service:</span>
					<code className="font-mono text-th-text-secondary">
						{health.notifyUrl}
					</code>
				</div>
			)}

			{/* Main grid: controls + environment status */}
			<div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
				<CheckControls
					triggering={triggering}
					lastTriggerResult={lastTriggerResult}
					lastTriggerAt={lastTriggerAt}
					triggerError={triggerError}
					autoTrigger={autoTrigger}
					autoTriggerInterval={autoTriggerInterval}
					onRunTrigger={runTrigger}
					onSetAutoTrigger={setAutoTrigger}
					onSetAutoTriggerInterval={setAutoTriggerInterval}
				/>
				<EnvironmentStatus tmux={tmux} lastCheckedAt={lastTriggerAt} />
			</div>

			{/* Answer success toast */}
			{answerSuccess && (
				<div className="rounded-md border border-th-status-success-border bg-th-status-success-bg px-4 py-3 text-sm text-th-status-success-text">
					Answer submitted successfully.
				</div>
			)}

			{/* Questions section */}
			<section aria-label="Questions">
				<div className="mb-3 flex items-center justify-between gap-3 flex-wrap">
					<h2 className="text-base font-semibold text-th-text">
						Questions
						{pendingCount > 0 && (
							<span className="ml-2 rounded-full bg-th-status-warning-bg px-2 py-0.5 text-xs font-medium text-th-status-warning-text">
								{pendingCount} pending
							</span>
						)}
					</h2>

					{/* Status filter */}
					<div
						className="flex items-center rounded-md border border-th-border overflow-hidden text-xs"
						role="group"
						aria-label="Filter by status"
					>
						{STATUS_FILTERS.map((filter) => (
							<button
								key={filter}
								type="button"
								onClick={() => setStatusFilter(filter)}
								aria-pressed={statusFilter === filter}
								className={[
									"px-3 py-1.5 transition-colors",
									statusFilter === filter
										? "bg-th-accent/10 text-th-text-link font-medium"
										: "text-th-text-muted hover:text-th-text",
								].join(" ")}
							>
								{filter}
							</button>
						))}
					</div>
				</div>

				{filteredQuestions.length === 0 ? (
					<div className="rounded-lg border border-dashed border-th-border bg-th-surface py-12 text-center">
						<HelpCircle
							size={32}
							className="mx-auto mb-3 text-th-text-faint"
						/>
						<p className="text-sm text-th-text-muted">
							{statusFilter === "All"
								? "No questions yet. Run checks to see if any action is needed."
								: `No ${statusFilter.toLowerCase()} questions.`}
						</p>
					</div>
				) : (
					<div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
						{filteredQuestions.map((question) => (
							<QuestionCard
								key={question.question_id}
								question={question}
								onAnswer={handleAnswer}
							/>
						))}
					</div>
				)}
			</section>

			{/* Answer dialog */}
			<AnswerDialog
				open={answerTarget !== null}
				question={answerTarget}
				answering={answering}
				answerError={answerError}
				onSubmit={(id, answer) => void handleSubmitAnswer(id, answer)}
				onClose={() => setAnswerTarget(null)}
			/>
		</div>
	);
}

export default QuestionList;
