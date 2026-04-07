/**
 * Tests for QuestionDetail page (v0.12.0 model).
 */

import { act, render, screen, waitFor } from "@testing-library/react";
import { fireEvent } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QuestionDetail } from "@/pages/questions/QuestionDetail";
import { askClient } from "@/services/ask";
import type { Question } from "@/types/ask";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeQuestion(overrides: Partial<Question> = {}): Question {
	return {
		id: "q-1",
		agent_id: "agent-abc",
		category: "deployment",
		question: "Should we proceed with the rollout?",
		priority: "High",
		status: "Pending",
		asked_at: "2024-06-01T12:00:00Z",
		...overrides,
	};
}

/** Render QuestionDetail at /questions/:id using MemoryRouter */
function renderDetail(id = "q-1") {
	return render(
		<MemoryRouter initialEntries={[`/questions/${id}`]}>
			<Routes>
				<Route path="/questions/:id" element={<QuestionDetail />} />
				<Route path="/questions" element={<div>Questions list</div>} />
			</Routes>
		</MemoryRouter>,
	);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("QuestionDetail", () => {
	beforeEach(() => {
		vi.restoreAllMocks();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	it("shows loading skeleton while fetching", () => {
		// Never resolves — stays in loading state
		vi.spyOn(askClient, "getQuestion").mockReturnValue(new Promise(() => {}));
		renderDetail();
		// Loading skeleton is an animated div (no specific text, but the back link should be present)
		expect(screen.getByText("Back to Questions")).toBeTruthy();
	});

	it("shows the question text after loading", async () => {
		vi.spyOn(askClient, "getQuestion").mockResolvedValue(makeQuestion());
		renderDetail();
		await waitFor(() =>
			expect(
				screen.getByText("Should we proceed with the rollout?"),
			).toBeTruthy(),
		);
	});

	it("shows the category", async () => {
		vi.spyOn(askClient, "getQuestion").mockResolvedValue(makeQuestion());
		renderDetail();
		await waitFor(() => expect(screen.getByText("deployment")).toBeTruthy());
	});

	it("shows the priority badge", async () => {
		vi.spyOn(askClient, "getQuestion").mockResolvedValue(makeQuestion());
		renderDetail();
		await waitFor(() => expect(screen.getByText("High")).toBeTruthy());
	});

	it("shows the agent ID", async () => {
		vi.spyOn(askClient, "getQuestion").mockResolvedValue(makeQuestion());
		renderDetail();
		await waitFor(() => expect(screen.getByText("agent-abc")).toBeTruthy());
	});

	it("shows the status badge", async () => {
		vi.spyOn(askClient, "getQuestion").mockResolvedValue(makeQuestion());
		renderDetail();
		await waitFor(() => expect(screen.getByText("Pending")).toBeTruthy());
	});

	it("shows Answer and Dismiss buttons for Pending questions", async () => {
		vi.spyOn(askClient, "getQuestion").mockResolvedValue(makeQuestion());
		renderDetail();
		await waitFor(() => expect(screen.getByText("Answer")).toBeTruthy());
		expect(screen.getByText("Dismiss")).toBeTruthy();
	});

	it("does not show Answer/Dismiss for Answered questions", async () => {
		vi.spyOn(askClient, "getQuestion").mockResolvedValue(
			makeQuestion({ status: "Answered", answer: "yes" }),
		);
		renderDetail();
		await waitFor(() => expect(screen.getByText("Answered")).toBeTruthy());
		expect(screen.queryByText("Answer")).toBeNull();
		expect(screen.queryByText("Dismiss")).toBeNull();
	});

	it("shows submitted answer for Answered questions", async () => {
		vi.spyOn(askClient, "getQuestion").mockResolvedValue(
			makeQuestion({ status: "Answered", answer: "yes, proceed" }),
		);
		renderDetail();
		await waitFor(() =>
			expect(screen.getByText("Answer submitted")).toBeTruthy(),
		);
		expect(screen.getByText("yes, proceed")).toBeTruthy();
	});

	it("shows context block when context is set", async () => {
		vi.spyOn(askClient, "getQuestion").mockResolvedValue(
			makeQuestion({ context: "Deploy to staging first" }),
		);
		renderDetail();
		await waitFor(() =>
			expect(screen.getByText("Deploy to staging first")).toBeTruthy(),
		);
	});

	it("shows optional workflow_id when present", async () => {
		vi.spyOn(askClient, "getQuestion").mockResolvedValue(
			makeQuestion({ workflow_id: "wf-123" }),
		);
		renderDetail();
		await waitFor(() => expect(screen.getByText("wf-123")).toBeTruthy());
	});

	it("shows optional dispatch_id when present", async () => {
		vi.spyOn(askClient, "getQuestion").mockResolvedValue(
			makeQuestion({ dispatch_id: "dp-456" }),
		);
		renderDetail();
		await waitFor(() => expect(screen.getByText("dp-456")).toBeTruthy());
	});

	it("shows error message when fetch fails", async () => {
		vi.spyOn(askClient, "getQuestion").mockRejectedValue(
			new Error("Network error"),
		);
		renderDetail();
		await waitFor(() =>
			expect(screen.getByText("Network error")).toBeTruthy(),
		);
	});

	it("shows retry button on error", async () => {
		vi.spyOn(askClient, "getQuestion").mockRejectedValue(
			new Error("Failed to load question"),
		);
		renderDetail();
		await waitFor(() => expect(screen.getByText("Retry")).toBeTruthy());
	});

	it("retries fetch when Retry is clicked", async () => {
		const spy = vi
			.spyOn(askClient, "getQuestion")
			.mockRejectedValueOnce(new Error("Temporary failure"))
			.mockResolvedValue(makeQuestion());
		renderDetail();
		await waitFor(() => expect(screen.getByText("Retry")).toBeTruthy());
		await act(async () => {
			fireEvent.click(screen.getByText("Retry"));
		});
		await waitFor(() =>
			expect(
				screen.getByText("Should we proceed with the rollout?"),
			).toBeTruthy(),
		);
		expect(spy).toHaveBeenCalledTimes(2);
	});

	it("opens answer dialog when Answer is clicked", async () => {
		vi.spyOn(askClient, "getQuestion").mockResolvedValue(makeQuestion());
		renderDetail();
		await waitFor(() => expect(screen.getByText("Answer")).toBeTruthy());
		fireEvent.click(screen.getByText("Answer"));
		expect(screen.getByRole("dialog")).toBeTruthy();
	});

	it("submits an answer via the dialog", async () => {
		vi.spyOn(askClient, "getQuestion").mockResolvedValue(makeQuestion());
		vi.spyOn(askClient, "answerQuestion").mockResolvedValue(
			makeQuestion({ id: "q-1", status: "Answered", answer: "yes, proceed" }),
		);
		renderDetail();
		await waitFor(() => expect(screen.getByText("Answer")).toBeTruthy());
		fireEvent.click(screen.getByText("Answer"));
		fireEvent.change(screen.getByLabelText("Custom answer"), {
			target: { value: "yes, proceed" },
		});
		await act(async () => {
			fireEvent.click(screen.getByText("Submit Answer"));
		});
		expect(askClient.answerQuestion).toHaveBeenCalledWith("q-1", {
			answer: "yes, proceed",
		});
	});

	it("calls dismissQuestion when Dismiss is clicked", async () => {
		vi.spyOn(askClient, "getQuestion").mockResolvedValue(makeQuestion());
		vi.spyOn(askClient, "dismissQuestion").mockResolvedValue(
			makeQuestion({ id: "q-1", status: "Dismissed" }),
		);
		renderDetail();
		await waitFor(() => expect(screen.getByText("Dismiss")).toBeTruthy());
		await act(async () => {
			fireEvent.click(screen.getByText("Dismiss"));
		});
		expect(askClient.dismissQuestion).toHaveBeenCalledWith("q-1");
	});

	it("renders a Back to Questions link", async () => {
		vi.spyOn(askClient, "getQuestion").mockResolvedValue(makeQuestion());
		renderDetail();
		const link = screen.getByText("Back to Questions").closest("a");
		expect(link).toBeTruthy();
		expect(link?.getAttribute("href")).toBe("/questions");
	});

	it("shows 'Question Detail' subtitle", async () => {
		vi.spyOn(askClient, "getQuestion").mockResolvedValue(makeQuestion());
		renderDetail();
		await waitFor(() =>
			expect(screen.getByText("Question Detail")).toBeTruthy(),
		);
	});
});
