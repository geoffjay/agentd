/**
 * Approval components -- unit tests for ApprovalBadge and ApprovalActions.
 *
 * The full ApprovalQueue page import causes OOM in the test worker due to
 * the @/components/common barrel pulling in a massive transitive dep graph
 * (HighlightedCode/prism, ServiceBanner/health, etc.). We test the approval
 * sub-components directly instead, which provides coverage without the OOM.
 */

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ApprovalActions } from "@/components/approvals/ApprovalActions";
import { ApprovalBadge } from "@/components/approvals/ApprovalBadge";

describe("ApprovalBadge", () => {
	it("renders the count", () => {
		render(<ApprovalBadge count={5} />);
		expect(screen.getByText("5")).toBeInTheDocument();
	});

	it("renders nothing when count is 0 and showZero is false", () => {
		const { container } = render(<ApprovalBadge count={0} />);
		expect(container.innerHTML).toBe("");
	});

	it("renders 0 when showZero is true", () => {
		render(<ApprovalBadge count={0} showZero />);
		expect(screen.getByText("0")).toBeInTheDocument();
	});

	it("renders 99+ for counts over 99", () => {
		render(<ApprovalBadge count={150} />);
		expect(screen.getByText("99+")).toBeInTheDocument();
	});

	it("sets correct aria-label for single approval", () => {
		render(<ApprovalBadge count={1} />);
		expect(screen.getByLabelText("1 pending approval")).toBeInTheDocument();
	});

	it("sets correct aria-label for multiple approvals", () => {
		render(<ApprovalBadge count={3} />);
		expect(screen.getByLabelText("3 pending approvals")).toBeInTheDocument();
	});

	it("applies pulse animation when count > 0", () => {
		render(<ApprovalBadge count={2} />);
		const badge = screen.getByText("2");
		expect(badge.className).toContain("animate-pulse");
	});
});

describe("ApprovalActions", () => {
	it("renders approve and deny buttons", () => {
		render(
			<ApprovalActions
				approvalId="ap-1"
				onApprove={vi.fn()}
				onDeny={vi.fn()}
			/>,
		);
		expect(screen.getByRole("button", { name: "Approve" })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Deny" })).toBeInTheDocument();
	});

	it("calls onApprove with the approval id", () => {
		const onApprove = vi.fn();
		render(
			<ApprovalActions
				approvalId="ap-1"
				onApprove={onApprove}
				onDeny={vi.fn()}
			/>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Approve" }));
		expect(onApprove).toHaveBeenCalledWith("ap-1");
	});

	it("calls onDeny with the approval id", () => {
		const onDeny = vi.fn();
		render(
			<ApprovalActions approvalId="ap-1" onApprove={vi.fn()} onDeny={onDeny} />,
		);
		fireEvent.click(screen.getByRole("button", { name: "Deny" }));
		expect(onDeny).toHaveBeenCalledWith("ap-1");
	});

	it("disables buttons when busy", () => {
		render(
			<ApprovalActions
				approvalId="ap-1"
				busy
				onApprove={vi.fn()}
				onDeny={vi.fn()}
			/>,
		);
		expect(screen.getByRole("button", { name: "Approve" })).toBeDisabled();
		expect(screen.getByRole("button", { name: "Deny" })).toBeDisabled();
	});

	it("shows Working text when busy", () => {
		render(
			<ApprovalActions
				approvalId="ap-1"
				busy
				onApprove={vi.fn()}
				onDeny={vi.fn()}
			/>,
		);
		expect(screen.getAllByText(/Working/)).toHaveLength(2);
	});
});
