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
			expect(badge.className).toContain("red");
		});

		it("renders service status healthy", () => {
			render(<StatusBadge status="healthy" />);
			const badge = screen.getByRole("status");
			expect(badge).toHaveTextContent("healthy");
			expect(badge.className).toContain("green");
		});

		it("renders service status down with red colour", () => {
			render(<StatusBadge status="down" />);
			const badge = screen.getByRole("status");
			expect(badge.className).toContain("red");
		});

		it("renders notification status pending", () => {
			render(<StatusBadge status="pending" />);
			expect(screen.getByRole("status")).toHaveTextContent("pending");
		});
	});

	describe("dot variant", () => {
		it("renders a coloured dot with aria-label", () => {
			render(<StatusBadge status="running" variant="dot" />);
			const dot = screen.getByRole("status", { name: "Running" });
			expect(dot.className).toContain("rounded-full");
			expect(dot.className).toContain("green");
		});

		it("applies correct colour for Failed", () => {
			render(<StatusBadge status="failed" variant="dot" />);
			const dot = screen.getByRole("status", { name: "Failed" });
			expect(dot.className).toContain("red");
		});

		it("applies custom className", () => {
			render(<StatusBadge status="running" variant="dot" className="ml-2" />);
			expect(screen.getByRole("status").className).toContain("ml-2");
		});
	});
});
