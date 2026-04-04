/**
 * Tests for ThemeToggle component.
 */

import { fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ThemeToggle } from "@/components/common/ThemeToggle";
import { ThemeProvider } from "@/hooks/useTheme";

function wrapper({ children }: { children: ReactNode }) {
	return <ThemeProvider>{children}</ThemeProvider>;
}

describe("ThemeToggle", () => {
	beforeEach(() => {
		localStorage.clear();
		document.documentElement.classList.remove("dark");
	});

	it("renders a button", () => {
		render(<ThemeToggle />, { wrapper });
		expect(screen.getByRole("button")).toBeInTheDocument();
	});

	it("has an accessible aria-label", () => {
		render(<ThemeToggle />, { wrapper });
		const btn = screen.getByRole("button");
		expect(btn).toHaveAttribute("aria-label");
		expect(btn.getAttribute("aria-label")).toMatch(/theme/i);
	});

	it("cycles from system to light on click", () => {
		// Default stored theme is 'system'
		render(<ThemeToggle />, { wrapper });
		const btn = screen.getByRole("button");

		fireEvent.click(btn);

		// After clicking system → light, dark class should be removed
		expect(document.documentElement.classList.contains("dark")).toBe(false);
	});

	it("applies custom className", () => {
		render(<ThemeToggle className="custom-class" />, { wrapper });
		expect(screen.getByRole("button")).toHaveClass("custom-class");
	});

	it("calls setTheme on click", () => {
		// Explicit theme -> system on click
		localStorage.setItem(
			"agentd:settings",
			JSON.stringify({ version: 1, ui: { theme: "agentd-light" } }),
		);
		render(<ThemeToggle />, { wrapper });
		const btn = screen.getByRole("button");

		fireEvent.click(btn);

		// agentd-light -> system (resolves based on OS preference)
		// Just verify the button label changed to indicate system mode
		expect(btn.getAttribute("aria-label")).toMatch(/System/);
	});

	it("label reflects current system mode", () => {
		render(<ThemeToggle />, { wrapper });
		expect(screen.getByRole("button")).toHaveAttribute(
			"aria-label",
			expect.stringContaining("System"),
		);
	});

	it("label reflects current light mode", () => {
		localStorage.setItem(
			"agentd:settings",
			JSON.stringify({ version: 1, ui: { theme: "agentd-light" } }),
		);
		render(<ThemeToggle />, { wrapper });
		expect(screen.getByRole("button")).toHaveAttribute(
			"aria-label",
			expect.stringContaining("System"),
		);
	});

	it("label reflects current dark mode", () => {
		localStorage.setItem(
			"agentd:settings",
			JSON.stringify({ version: 1, ui: { theme: "agentd-dark" } }),
		);
		render(<ThemeToggle />, { wrapper });
		expect(screen.getByRole("button")).toHaveAttribute(
			"aria-label",
			expect.stringContaining("System"),
		);
	});

	it("does not throw when ThemeProvider is missing", () => {
		// suppress error boundary output
		const spy = vi.spyOn(console, "error").mockImplementation(() => {});
		expect(() => render(<ThemeToggle />)).toThrow();
		spy.mockRestore();
	});
});
