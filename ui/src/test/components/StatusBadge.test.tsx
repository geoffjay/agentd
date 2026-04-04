import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { StatusBadge } from "@/components/common/StatusBadge";

describe("StatusBadge", () => {
	describe("badge variant (default)", () => {
		it("renders agent status running", () => {
			render(<StatusBadge status="running" />);
			expect(screen.getByRole("status")).toHaveTextContent("running");
		});

		it("renders agent status failed", () => {
			render(<StatusBadge status="failed" />);
			const badge = screen.getByRole("status");
			expect(badge).toHaveTextContent("failed");
			expect(badge.className).toContain("bg-th-status-error-bg");
		});

		it("renders service status healthy", () => {
			render(<StatusBadge status="healthy" />);
			const badge = screen.getByRole("status");
			expect(badge).toHaveTextContent("healthy");
			expect(badge.className).toContain("bg-th-status-success-bg");
		});

		it("renders service status down with error colour", () => {
			render(<StatusBadge status="down" />);
			const badge = screen.getByRole("status");
			expect(badge.className).toContain("bg-th-status-error-bg");
		});

		it("renders notification status pending", () => {
			render(<StatusBadge status="pending" />);
			expect(screen.getByRole("status")).toHaveTextContent("pending");
		});
	});

	describe("dot variant", () => {
		it("renders a coloured dot with aria-label", () => {
			render(<StatusBadge status="running" variant="dot" />);
			const dot = screen.getByRole("status", { name: "running" });
			expect(dot.className).toContain("rounded-full");
			expect(dot.className).toContain("bg-th-status-success-dot");
		});

		it("applies correct colour for failed", () => {
			render(<StatusBadge status="failed" variant="dot" />);
			const dot = screen.getByRole("status", { name: "failed" });
			expect(dot.className).toContain("bg-th-status-error-dot");
		});

		it("applies custom className", () => {
			render(<StatusBadge status="running" variant="dot" className="ml-2" />);
			expect(screen.getByRole("status").className).toContain("ml-2");
		});
	});
});
