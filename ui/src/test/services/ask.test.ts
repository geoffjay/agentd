import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AskClient } from "@/services/ask";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeJsonResponse(status: number, body: unknown) {
	return new Response(JSON.stringify(body), {
		status,
		headers: new Headers({ "content-type": "application/json" }),
	});
}

function mockFetch(status: number, body: unknown) {
	vi.stubGlobal(
		"fetch",
		vi.fn().mockResolvedValue(makeJsonResponse(status, body)),
	);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("AskClient", () => {
	let client: AskClient;

	beforeEach(() => {
		client = new AskClient({
			baseUrl: "http://localhost:17001",
			maxRetries: 1,
		});
	});

	afterEach(() => {
		vi.unstubAllGlobals();
	});

	describe("getHealth", () => {
		it("calls GET /health", async () => {
			mockFetch(200, { service: "ask", version: "0.12.0", status: "ok" });
			const result = await client.getHealth();
			expect(result.service).toBe("ask");

			const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string;
			expect(calledUrl).toContain("/health");
		});
	});

	describe("listQuestions", () => {
		it("calls GET /questions and returns paginated result", async () => {
			const mockResponse = {
				items: [
					{
						id: "q-1",
						agent_id: "agent-1",
						category: "general",
						question: "Is the deployment ready?",
						priority: "Normal",
						status: "Pending",
						asked_at: "2024-01-01T00:00:00Z",
					},
				],
				total: 1,
				limit: 20,
				offset: 0,
			};
			mockFetch(200, mockResponse);

			const result = await client.listQuestions();
			expect(result.items).toHaveLength(1);
			expect(result.total).toBe(1);

			const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string;
			expect(calledUrl).toContain("/questions");
		});

		it("passes query params as URL search params", async () => {
			mockFetch(200, { items: [], total: 0, limit: 20, offset: 0 });

			await client.listQuestions({ status: "Pending", limit: 10 });

			const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string;
			expect(calledUrl).toContain("status=Pending");
			expect(calledUrl).toContain("limit=10");
		});
	});

	describe("getQuestion", () => {
		it("calls GET /questions/:id", async () => {
			const mockQuestion = {
				id: "q-123",
				agent_id: "agent-1",
				category: "deployment",
				question: "Proceed with rollout?",
				priority: "High",
				status: "Pending",
				asked_at: "2024-01-01T00:00:00Z",
			};
			mockFetch(200, mockQuestion);

			const result = await client.getQuestion("q-123");
			expect(result.id).toBe("q-123");
			expect(result.priority).toBe("High");

			const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string;
			expect(calledUrl).toContain("/questions/q-123");
		});
	});

	describe("answerQuestion", () => {
		it("calls POST /questions/:id/answer", async () => {
			mockFetch(200, {
				success: true,
				message: "Answer recorded",
				question_id: "q-1",
			});

			const result = await client.answerQuestion("q-1", { answer: "yes" });
			expect(result.success).toBe(true);
			expect(result.question_id).toBe("q-1");

			const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string;
			expect(calledUrl).toContain("/questions/q-1/answer");

			const callInit = vi.mocked(fetch).mock.calls[0][1] as RequestInit;
			expect(callInit.method).toBe("POST");

			const body = JSON.parse(callInit.body as string);
			expect(body.answer).toBe("yes");
		});

		it("handles failure response", async () => {
			mockFetch(200, {
				success: false,
				message: "Question already answered",
				question_id: "q-1",
			});

			const result = await client.answerQuestion("q-1", { answer: "no" });
			expect(result.success).toBe(false);
		});
	});

	describe("dismissQuestion", () => {
		it("calls POST /questions/:id/dismiss", async () => {
			mockFetch(200, {
				success: true,
				message: "Question dismissed",
				question_id: "q-2",
			});

			const result = await client.dismissQuestion("q-2");
			expect(result.success).toBe(true);

			const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string;
			expect(calledUrl).toContain("/questions/q-2/dismiss");

			const callInit = vi.mocked(fetch).mock.calls[0][1] as RequestInit;
			expect(callInit.method).toBe("POST");
		});
	});
});
