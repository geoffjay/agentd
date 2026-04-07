/**
 * TypeScript types for the Ask service.
 * Mirrors the Rust types in crates/ask.
 */

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/** Priority level of a question */
export type QuestionPriority = "Low" | "Normal" | "High" | "Urgent";

/** Current state of a question */
export type QuestionStatus = "Pending" | "Answered" | "Dismissed" | "Expired";

// ---------------------------------------------------------------------------
// Question model
// ---------------------------------------------------------------------------

/** A question posed by an agent awaiting a human response */
export interface Question {
	id: string;
	agent_id: string;
	workflow_id?: string;
	dispatch_id?: string;
	category: string;
	question: string;
	context?: string;
	priority: QuestionPriority;
	status: QuestionStatus;
	answer?: string;
	asked_at: string;
	answered_at?: string;
	expires_at?: string;
}

// ---------------------------------------------------------------------------
// Request / response bodies
// ---------------------------------------------------------------------------

/** Body for POST /questions (agent creates a question) */
export interface CreateQuestionRequest {
	agent_id: string;
	workflow_id?: string;
	dispatch_id?: string;
	category: string;
	question: string;
	context?: string;
	priority?: QuestionPriority;
	expires_at?: string;
}

/** Body for POST /questions/{id}/answer */
export interface AnswerQuestionRequest {
	answer: string;
}

/** Response from answer / dismiss endpoints */
export interface QuestionActionResponse {
	success: boolean;
	message: string;
	question_id: string;
}

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

/** Query parameters for GET /questions */
export interface ListQuestionsParams {
	status?: QuestionStatus;
	agent_id?: string;
	category?: string;
	limit?: number;
	offset?: number;
}

// ---------------------------------------------------------------------------
// Legacy types — kept for backwards compatibility during migration.
// Will be removed once all components are updated.
// ---------------------------------------------------------------------------

/** @deprecated Use QuestionStatus instead */
export type CheckType = "TmuxSessions";

/** @deprecated Use Question instead */
export interface QuestionInfo {
	question_id: string;
	notification_id: string;
	check_type: CheckType;
	asked_at: string;
	status: QuestionStatus;
	answer?: string;
}

/** @deprecated Legacy tmux check result */
export interface TmuxCheckResult {
	running: boolean;
	session_count: number;
	sessions?: string[];
}

/** @deprecated Legacy trigger results */
export interface TriggerResults {
	tmux_sessions: TmuxCheckResult;
}

/** @deprecated Response from old POST /trigger */
export interface TriggerResponse {
	checks_run: string[];
	notifications_sent: string[];
	results: TriggerResults;
}

/** @deprecated Use AnswerQuestionRequest instead */
export interface AnswerRequest {
	question_id: string;
	answer: string;
}

/** @deprecated Use QuestionActionResponse instead */
export interface AnswerResponse {
	success: boolean;
	message: string;
	question_id: string;
}
