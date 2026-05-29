/**
 * Tests for useTheme hook and ThemeProvider.
 *
 * These are unit tests that verify:
 * - ThemeProvider reads initial theme from settingsStore
 * - setTheme updates DOM and persists to localStorage
 * - System mode responds to prefers-color-scheme changes
 * - resolvedThemeId correctly reflects the active theme
 */

import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ThemeProvider, useTheme } from "@/hooks/useTheme";

// ---------------------------------------------------------------------------
// Test wrapper
// ---------------------------------------------------------------------------

function wrapper({ children }: { children: ReactNode }) {
	return <ThemeProvider>{children}</ThemeProvider>;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function setStoredTheme(theme: string) {
	localStorage.setItem(
		"agentd:settings",
		JSON.stringify({ version: 1, ui: { theme } }),
	);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("useTheme", () => {
	beforeEach(() => {
		localStorage.clear();
		document.documentElement.removeAttribute("data-theme");
	});

	afterEach(() => {
		vi.restoreAllMocks();
		document.documentElement.removeAttribute("data-theme");
	});

	it("defaults to system theme when no preference is stored", () => {
		const { result } = renderHook(() => useTheme(), { wrapper });
		expect(result.current.themeId).toBe("system");
	});

	it("reads stored named theme preference", () => {
		setStoredTheme("tokyo-night");
		const { result } = renderHook(() => useTheme(), { wrapper });
		expect(result.current.themeId).toBe("tokyo-night");
	});

	it("sets data-theme for a dark theme", () => {
		setStoredTheme("agentd-dark");
		renderHook(() => useTheme(), { wrapper });
		expect(document.documentElement.getAttribute("data-theme")).toBe(
			"agentd-dark",
		);
	});

	it("sets data-theme for a light theme", () => {
		setStoredTheme("agentd-light");
		renderHook(() => useTheme(), { wrapper });
		expect(document.documentElement.getAttribute("data-theme")).toBe(
			"agentd-light",
		);
	});

	it("setTheme updates the theme and applies to DOM", () => {
		const { result } = renderHook(() => useTheme(), { wrapper });

		act(() => {
			result.current.setTheme("kanagawa");
		});

		expect(result.current.themeId).toBe("kanagawa");
		expect(document.documentElement.getAttribute("data-theme")).toBe(
			"kanagawa",
		);
	});

	it("setTheme to light theme sets correct data-theme", () => {
		const { result } = renderHook(() => useTheme(), { wrapper });

		act(() => {
			result.current.setTheme("agentd-light");
		});

		expect(result.current.themeId).toBe("agentd-light");
		expect(document.documentElement.getAttribute("data-theme")).toBe(
			"agentd-light",
		);
	});

	it("setTheme persists to localStorage", () => {
		const { result } = renderHook(() => useTheme(), { wrapper });

		act(() => {
			result.current.setTheme("catppuccin-mocha");
		});

		const stored = JSON.parse(localStorage.getItem("agentd:settings") ?? "{}");
		expect(stored.ui.theme).toBe("catppuccin-mocha");
	});

	it("resolvedThemeId is concrete for named themes", () => {
		setStoredTheme("tokyo-night");
		const { result } = renderHook(() => useTheme(), { wrapper });
		expect(result.current.resolvedThemeId).toBe("tokyo-night");
	});

	it("resolvedThemeId follows system preference in system mode", () => {
		Object.defineProperty(window, "matchMedia", {
			writable: true,
			value: vi.fn().mockImplementation((query: string) => ({
				matches: query === "(prefers-color-scheme: dark)",
				media: query,
				onchange: null,
				addListener: vi.fn(),
				removeListener: vi.fn(),
				addEventListener: vi.fn(),
				removeEventListener: vi.fn(),
				dispatchEvent: vi.fn(),
			})),
		});

		const { result } = renderHook(() => useTheme(), { wrapper });
		expect(result.current.resolvedThemeId).toBe("agentd-dark");
	});

	it("theme object has correct family", () => {
		setStoredTheme("catppuccin-latte");
		const { result } = renderHook(() => useTheme(), { wrapper });
		expect(result.current.theme.family).toBe("light");
	});

	it("throws when used outside ThemeProvider", () => {
		const spy = vi.spyOn(console, "error").mockImplementation(() => {});
		expect(() => renderHook(() => useTheme())).toThrow(
			"useTheme must be used within a ThemeProvider",
		);
		spy.mockRestore();
	});
});
