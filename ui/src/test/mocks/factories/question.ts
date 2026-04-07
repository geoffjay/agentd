/**
 * Test data factory for Ask service types.
 *
 * Usage:
 *   const question = makeQuestion()
 *   const response = makeQuestionActionResponse()
 */

import type {
	AnswerResponse,
	Question,
	QuestionActionResponse,
	QuestionInfo,
	QuestionPriority,
	QuestionStatus,
	TriggerResponse,
} from "@/types/ask";

let _seq = 0;

/** Reset the sequence counter (call in beforeEach to get predictable IDs) */
export function resetQuestionSeq(): void {
	_seq = 0;
}

// ---------------------------------------------------------------------------
// New Question factory
// ---------------------------------------------------------------------------

export function makeQuestion(overrides?: Partial<Question>): Question {
	const id = ++_seq;
	return {
		id: String(id),
		agent_id: `agent-${id}`,
		category: "general",
		question: `Test question ${id}?`,
		priority: "Normal" as QuestionPriority,
		status: "Pending" as QuestionStatus,
		asked_at: "2024-01-01T00:00:00Z",
		...overrides,
	};
}

export function makeQuestionActionResponse(
	overrides?: Partial<QuestionActionResponse>,
): QuestionActionResponse {
	return {
		success: true,
		message: "Action completed successfully",
		question_id: String(_seq),
		...overrides,
	};
}

// ---------------------------------------------------------------------------
// Legacy factories — kept for backwards compatibility during migration.
// Will be removed in #1014.
// ---------------------------------------------------------------------------

/** @deprecated Use makeQuestion instead */
export function makeQuestionInfo(
	overrides?: Partial<QuestionInfo>,
): QuestionInfo {
	const id = ++_seq;
	return {
		question_id: String(id),
		notification_id: String(++_seq),
		check_type: "TmuxSessions",
		asked_at: "2024-01-01T00:00:00Z",
		status: "Pending",
		...overrides,
	};
}

/** @deprecated Legacy trigger response factory */
export function makeTriggerResponse(
	overrides?: Partial<TriggerResponse>,
): TriggerResponse {
	return {
		checks_run: ["TmuxSessions"],
		notifications_sent: [],
		results: {
			tmux_sessions: {
				running: true,
				session_count: 2,
				sessions: ["main", "dev"],
			},
		},
		...overrides,
	};
}

/** @deprecated Use makeQuestionActionResponse instead */
export function makeAnswerResponse(
	overrides?: Partial<AnswerResponse>,
): AnswerResponse {
	const id = ++_seq;
	return {
		success: true,
		message: "Answer recorded successfully",
		question_id: String(id),
		...overrides,
	};
}
