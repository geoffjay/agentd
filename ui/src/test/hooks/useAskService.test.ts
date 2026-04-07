/**
 * Tests for useAskService hook (v0.12.0 Q&A model).
 */

import { act, renderHook, waitFor } from "@testing-library/react";
import { HttpResponse, http } from "msw";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAskService } from "@/hooks/useAskService";
import { askClient } from "@/services/ask";
import { makeQuestion, makeQuestionActionResponse, resetQuestionSeq } from "@/test/mocks/factories";
import { server } from "@/test/mocks/server";

const BASE = "http://localhost:17001";

function makePaginatedList(items = [makeQuestion()]) {
	return { items, total: items.length, limit: 20, offset: 0 };
}

beforeEach(() => {
	resetQuestionSeq();
	vi.restoreAllMocks();
});

describe("useAskService", () => {
	describe("health", () => {
		it("reports reachable when health check succeeds", async () => {
			vi.spyOn(askClient, "listQuestions").mockResolvedValue(
				makePaginatedList([]),
			);
			const { result } = renderHook(() => useAskService());
			await waitFor(() => expect(result.current.health.reachable).toBe(true));
			expect(result.current.health.checking).toBe(false);
		});

		it("reports unreachable when health check fails", async () => {
			vi.spyOn(askClient, "listQuestions").mockResolvedValue(
				makePaginatedList([]),
			);
			server.use(http.get(`${BASE}/health`, () => HttpResponse.error()));
			const { result } = renderHook(() => useAskService());
			await waitFor(() => expect(result.current.health.checking).toBe(false));
			expect(result.current.health.reachable).toBe(false);
		});

		it("recheckHealth re-runs the health check", async () => {
			vi.spyOn(askClient, "listQuestions").mockResolvedValue(
				makePaginatedList([]),
			);
			const { result } = renderHook(() => useAskService());
			await waitFor(() => expect(result.current.health.reachable).toBe(true));

			server.use(http.get(`${BASE}/health`, () => HttpResponse.error()));
			act(() => {
				result.current.recheckHealth();
			});
			await waitFor(() => expect(result.current.health.reachable).toBe(false));
		});
	});

	describe("questions loading", () => {
		it("fetches questions on mount", async () => {
			const q = makeQuestion();
			vi.spyOn(askClient, "listQuestions").mockResolvedValue(
				makePaginatedList([q]),
			);

			const { result } = renderHook(() => useAskService());
			await waitFor(() => expect(result.current.loading).toBe(false));

			expect(result.current.questions).toHaveLength(1);
			expect(result.current.questions[0].id).toBe(q.id);
			expect(result.current.total).toBe(1);
		});

		it("sets error when fetch fails", async () => {
			vi.spyOn(askClient, "listQuestions").mockRejectedValue(
				new Error("Network error"),
			);

			const { result } = renderHook(() => useAskService());
			await waitFor(() => expect(result.current.loading).toBe(false));
			expect(result.current.error).toBeDefined();
		});

		it("refetch re-loads questions", async () => {
			const spy = vi
				.spyOn(askClient, "listQuestions")
				.mockResolvedValue(makePaginatedList([]));

			const { result } = renderHook(() => useAskService());
			await waitFor(() => expect(result.current.loading).toBe(false));
			expect(spy).toHaveBeenCalledTimes(1);

			await act(async () => {
				result.current.refetch();
			});
			expect(spy.mock.calls.length).toBeGreaterThanOrEqual(2);
		});
	});

	describe("answerQuestion", () => {
		it("marks question as Answered on success", async () => {
			const q = makeQuestion({ status: "Pending" });
			vi.spyOn(askClient, "listQuestions").mockResolvedValue(
				makePaginatedList([q]),
			);
			vi.spyOn(askClient, "answerQuestion").mockResolvedValue(
				makeQuestionActionResponse({ success: true, question_id: q.id }),
			);

			const { result } = renderHook(() => useAskService());
			await waitFor(() => expect(result.current.loading).toBe(false));

			let success = false;
			await act(async () => {
				success = await result.current.answerQuestion(q.id, "yes");
			});

			expect(success).toBe(true);
			await waitFor(() =>
				expect(
					result.current.questions.find((q2) => q2.id === q.id)?.status,
				).toBe("Answered"),
			);
		});

		it("sets actionError on network failure", async () => {
			const q = makeQuestion({ status: "Pending" });
			vi.spyOn(askClient, "listQuestions").mockResolvedValue(
				makePaginatedList([q]),
			);
			vi.spyOn(askClient, "answerQuestion").mockRejectedValue(
				new Error("Network error"),
			);

			const { result } = renderHook(() => useAskService());
			await waitFor(() => expect(result.current.loading).toBe(false));

			let success = true;
			await act(async () => {
				success = await result.current.answerQuestion(q.id, "yes");
			});

			expect(success).toBe(false);
			expect(result.current.actionError).toBeDefined();
		});
	});

	describe("dismissQuestion", () => {
		it("marks question as Dismissed on success", async () => {
			const q = makeQuestion({ status: "Pending" });
			vi.spyOn(askClient, "listQuestions").mockResolvedValue(
				makePaginatedList([q]),
			);
			vi.spyOn(askClient, "dismissQuestion").mockResolvedValue(
				makeQuestionActionResponse({ success: true, question_id: q.id }),
			);

			const { result } = renderHook(() => useAskService());
			await waitFor(() => expect(result.current.loading).toBe(false));

			let success = false;
			await act(async () => {
				success = await result.current.dismissQuestion(q.id);
			});

			expect(success).toBe(true);
			await waitFor(() =>
				expect(
					result.current.questions.find((q2) => q2.id === q.id)?.status,
				).toBe("Dismissed"),
			);
		});

		it("sets actionError on failure", async () => {
			const q = makeQuestion({ status: "Pending" });
			vi.spyOn(askClient, "listQuestions").mockResolvedValue(
				makePaginatedList([q]),
			);
			vi.spyOn(askClient, "dismissQuestion").mockRejectedValue(
				new Error("Network error"),
			);

			const { result } = renderHook(() => useAskService());
			await waitFor(() => expect(result.current.loading).toBe(false));

			let success = true;
			await act(async () => {
				success = await result.current.dismissQuestion(q.id);
			});

			expect(success).toBe(false);
			expect(result.current.actionError).toBeDefined();
		});
	});

	describe("filters", () => {
		it("starts with empty filters", async () => {
			vi.spyOn(askClient, "listQuestions").mockResolvedValue(
				makePaginatedList([]),
			);
			const { result } = renderHook(() => useAskService());
			await waitFor(() => expect(result.current.loading).toBe(false));
			expect(result.current.filters).toEqual({});
		});

		it("setStatusFilter updates filters", async () => {
			vi.spyOn(askClient, "listQuestions").mockResolvedValue(
				makePaginatedList([]),
			);
			const { result } = renderHook(() => useAskService());
			await waitFor(() => expect(result.current.loading).toBe(false));

			act(() => result.current.setStatusFilter("Pending"));
			expect(result.current.filters.status).toBe("Pending");
		});

		it("setFilters replaces all filters", async () => {
			vi.spyOn(askClient, "listQuestions").mockResolvedValue(
				makePaginatedList([]),
			);
			const { result } = renderHook(() => useAskService());
			await waitFor(() => expect(result.current.loading).toBe(false));

			act(() =>
				result.current.setFilters({ status: "Answered", category: "deploy" }),
			);
			expect(result.current.filters.status).toBe("Answered");
			expect(result.current.filters.category).toBe("deploy");
		});
	});

	describe("polling", () => {
		it("starts with polling disabled when no pending questions", async () => {
			vi.spyOn(askClient, "listQuestions").mockResolvedValue(
				makePaginatedList([makeQuestion({ status: "Answered" })]),
			);
			const { result } = renderHook(() => useAskService());
			await waitFor(() => expect(result.current.loading).toBe(false));
			expect(result.current.pollingEnabled).toBe(false);
		});

		it("auto-enables polling when there are pending questions", async () => {
			vi.spyOn(askClient, "listQuestions").mockResolvedValue(
				makePaginatedList([makeQuestion({ status: "Pending" })]),
			);
			const { result } = renderHook(() => useAskService());
			await waitFor(() => expect(result.current.pollingEnabled).toBe(true));
		});

		it("setPollingEnabled toggles polling on and off", async () => {
			vi.spyOn(askClient, "listQuestions").mockResolvedValue(
				makePaginatedList([]),
			);
			const { result } = renderHook(() => useAskService());
			await waitFor(() => expect(result.current.loading).toBe(false));

			act(() => result.current.setPollingEnabled(true));
			expect(result.current.pollingEnabled).toBe(true);

			act(() => result.current.setPollingEnabled(false));
			expect(result.current.pollingEnabled).toBe(false);
		});

		it("setPollingInterval updates the interval", async () => {
			vi.spyOn(askClient, "listQuestions").mockResolvedValue(
				makePaginatedList([]),
			);
			const { result } = renderHook(() => useAskService());
			await waitFor(() => expect(result.current.loading).toBe(false));

			act(() => result.current.setPollingInterval(30_000));
			expect(result.current.pollingInterval).toBe(30_000);
		});
	});
});
