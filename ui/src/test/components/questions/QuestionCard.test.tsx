/**
 * Tests for QuestionCard component (v0.12.0 model).
 */

import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { QuestionCard } from "@/components/questions/QuestionCard";
import type { Question } from "@/types/ask";

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

function renderCard(props: Parameters<typeof QuestionCard>[0]) {
	return render(
		<MemoryRouter>
			<QuestionCard {...props} />
		</MemoryRouter>,
	);
}

describe("QuestionCard", () => {
	it("renders the question text", () => {
		renderCard({ question: makeQuestion(), onAnswer: vi.fn() });
		expect(screen.getByText("Should we proceed with the rollout?")).toBeTruthy();
	});

	it("renders the category", () => {
		renderCard({ question: makeQuestion(), onAnswer: vi.fn() });
		expect(screen.getByText("deployment")).toBeTruthy();
	});

	it("renders the priority badge", () => {
		renderCard({ question: makeQuestion(), onAnswer: vi.fn() });
		expect(screen.getByText("High")).toBeTruthy();
	});

	it("renders the agent ID", () => {
		renderCard({ question: makeQuestion(), onAnswer: vi.fn() });
		expect(screen.getByText("agent-abc")).toBeTruthy();
	});

	it('shows "Pending" status badge', () => {
		renderCard({
			question: makeQuestion({ status: "Pending" }),
			onAnswer: vi.fn(),
		});
		expect(screen.getByText("Pending")).toBeTruthy();
	});

	it('shows "Answered" status badge', () => {
		renderCard({
			question: makeQuestion({ status: "Answered" }),
			onAnswer: vi.fn(),
		});
		expect(screen.getByText("Answered")).toBeTruthy();
	});

	it('shows "Dismissed" status badge', () => {
		renderCard({
			question: makeQuestion({ status: "Dismissed" }),
			onAnswer: vi.fn(),
		});
		expect(screen.getByText("Dismissed")).toBeTruthy();
	});

	it('shows "Expired" status badge', () => {
		renderCard({
			question: makeQuestion({ status: "Expired" }),
			onAnswer: vi.fn(),
		});
		expect(screen.getByText("Expired")).toBeTruthy();
	});

	it("shows Answer button for Pending questions", () => {
		renderCard({
			question: makeQuestion({ status: "Pending" }),
			onAnswer: vi.fn(),
		});
		expect(screen.getByText("Answer")).toBeTruthy();
	});

	it("shows Dismiss button when onDismiss is provided for Pending questions", () => {
		renderCard({
			question: makeQuestion({ status: "Pending" }),
			onAnswer: vi.fn(),
			onDismiss: vi.fn(),
		});
		expect(screen.getByText("Dismiss")).toBeTruthy();
	});

	it("does not show action buttons for Answered questions", () => {
		renderCard({
			question: makeQuestion({ status: "Answered" }),
			onAnswer: vi.fn(),
		});
		expect(screen.queryByText("Answer")).toBeNull();
		expect(screen.queryByText("Dismiss")).toBeNull();
	});

	it("does not show action buttons for Expired questions", () => {
		renderCard({
			question: makeQuestion({ status: "Expired" }),
			onAnswer: vi.fn(),
		});
		expect(screen.queryByText("Answer")).toBeNull();
	});

	it("calls onAnswer with the question when Answer is clicked", () => {
		const onAnswer = vi.fn();
		const question = makeQuestion();
		renderCard({ question, onAnswer });
		fireEvent.click(screen.getByText("Answer"));
		expect(onAnswer).toHaveBeenCalledWith(question);
	});

	it("calls onDismiss with the question when Dismiss is clicked", () => {
		const onDismiss = vi.fn();
		const question = makeQuestion();
		renderCard({ question, onAnswer: vi.fn(), onDismiss });
		fireEvent.click(screen.getByText("Dismiss"));
		expect(onDismiss).toHaveBeenCalledWith(question);
	});

	it("shows submitted answer text for Answered questions", () => {
		renderCard({
			question: makeQuestion({ status: "Answered", answer: "yes, proceed" }),
			onAnswer: vi.fn(),
		});
		expect(screen.getByText("Answer submitted")).toBeTruthy();
		expect(screen.getByText("yes, proceed")).toBeTruthy();
	});

	it("shows context toggle when context is provided", () => {
		renderCard({
			question: makeQuestion({ context: "Some background info" }),
			onAnswer: vi.fn(),
		});
		expect(screen.getByText(/Show context/)).toBeTruthy();
	});

	it("expands context when toggle is clicked", () => {
		renderCard({
			question: makeQuestion({ context: "Some background info" }),
			onAnswer: vi.fn(),
		});
		fireEvent.click(screen.getByText(/Show context/));
		expect(screen.getByText("Some background info")).toBeTruthy();
	});

	it("renders a link to the detail view", () => {
		renderCard({ question: makeQuestion(), onAnswer: vi.fn() });
		const link = screen.getByText("View detail").closest("a");
		expect(link).toBeTruthy();
		expect(link?.getAttribute("href")).toBe("/questions/q-1");
	});
});
