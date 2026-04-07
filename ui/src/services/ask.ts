/**
 * Client for the Ask service (default port 17001).
 *
 * Provides the agent-to-human Q&A workflow introduced in v0.12.0:
 * - List / get questions
 * - Answer or dismiss a pending question
 */

import type { HealthResponse, PaginatedResponse } from "@/types/common";
import type {
	AnswerQuestionRequest,
	AnswerRequest,
	AnswerResponse,
	CreateQuestionRequest,
	ListQuestionsParams,
	Question,
	QuestionActionResponse,
	TriggerResponse,
} from "@/types/ask";
import { ApiClient } from "./base";
import { serviceConfig } from "./config";

export class AskClient extends ApiClient {
	// -------------------------------------------------------------------------
	// Health
	// -------------------------------------------------------------------------

	getHealth(): Promise<HealthResponse> {
		return this.get<HealthResponse>("/health");
	}

	// -------------------------------------------------------------------------
	// Questions
	// -------------------------------------------------------------------------

	/** List questions with optional filters */
	listQuestions(
		params?: ListQuestionsParams,
	): Promise<PaginatedResponse<Question>> {
		return this.get<PaginatedResponse<Question>>(
			"/questions",
			params as Record<string, string>,
		);
	}

	/** Get a single question by ID */
	getQuestion(id: string): Promise<Question> {
		return this.get<Question>(`/questions/${id}`);
	}

	/** Create a new question (called by agents) */
	createQuestion(request: CreateQuestionRequest): Promise<Question> {
		return this.post<Question>("/questions", request);
	}

	/** Submit an answer to a pending question */
	answerQuestion(
		id: string,
		request: AnswerQuestionRequest,
	): Promise<QuestionActionResponse> {
		return this.post<QuestionActionResponse>(
			`/questions/${id}/answer`,
			request,
		);
	}

	/** Dismiss a pending question */
	dismissQuestion(id: string): Promise<QuestionActionResponse> {
		return this.post<QuestionActionResponse>(`/questions/${id}/dismiss`);
	}

	// -------------------------------------------------------------------------
	// Legacy methods — kept for backwards compatibility during migration.
	// Will be removed in #1009 when useAskService is rewritten.
	// -------------------------------------------------------------------------

	/**
	 * @deprecated Use listQuestions / answerQuestion / dismissQuestion instead.
	 * Run all environment checks.
	 */
	trigger(): Promise<TriggerResponse> {
		return this.post<TriggerResponse>("/trigger");
	}

	/**
	 * @deprecated Use answerQuestion(id, { answer }) instead.
	 * Submit an answer to a pending question via the old endpoint.
	 */
	answer(request: AnswerRequest): Promise<AnswerResponse> {
		return this.post<AnswerResponse>("/answer", request);
	}
}

/** Singleton client instance using the configured service URL */
export const askClient = new AskClient({
	baseUrl: serviceConfig.askServiceUrl,
});
