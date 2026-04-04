/**
 * Tests for themeStore pure functions.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
	applyTheme,
	readPersistedTheme,
	resolveThemeId,
} from "@/stores/themeStore";

function mockMatchMedia(prefersDark: boolean) {
	Object.defineProperty(window, "matchMedia", {
		writable: true,
		value: vi.fn().mockImplementation((query: string) => ({
			matches: prefersDark && query === "(prefers-color-scheme: dark)",
			media: query,
			onchange: null,
			addListener: vi.fn(),
			removeListener: vi.fn(),
			addEventListener: vi.fn(),
			removeEventListener: vi.fn(),
			dispatchEvent: vi.fn(),
		})),
	});
}

describe("resolveThemeId", () => {
	afterEach(() => vi.restoreAllMocks());

	it("returns the same ID for a known theme", () => {
		expect(resolveThemeId("tokyo-night")).toBe("tokyo-night");
	});

	it("returns default for unknown theme ID", () => {
		expect(resolveThemeId("nonexistent")).toBe("agentd-dark");
	});

	it("returns agentd-dark when system prefers dark", () => {
		mockMatchMedia(true);
		expect(resolveThemeId("system")).toBe("agentd-dark");
	});

	it("returns agentd-light when system prefers light", () => {
		mockMatchMedia(false);
		expect(resolveThemeId("system")).toBe("agentd-light");
	});
});

describe("applyTheme", () => {
	beforeEach(() => {
		document.documentElement.removeAttribute("data-theme");
	});

	afterEach(() => {
		document.documentElement.removeAttribute("data-theme");
		vi.restoreAllMocks();
	});

	it("sets data-theme attribute for a dark theme", () => {
		applyTheme("agentd-dark");
		expect(document.documentElement.getAttribute("data-theme")).toBe("agentd-dark");
	});

	it("sets data-theme attribute for a light theme", () => {
		applyTheme("agentd-light");
		expect(document.documentElement.getAttribute("data-theme")).toBe("agentd-light");
	});

	it("sets data-theme for named themes", () => {
		applyTheme("tokyo-night");
		expect(document.documentElement.getAttribute("data-theme")).toBe("tokyo-night");
	});

	it("sets color-scheme to dark for dark themes", () => {
		applyTheme("kanagawa");
		expect(document.documentElement.style.colorScheme).toBe("dark");
	});

	it("sets color-scheme to light for light themes", () => {
		applyTheme("nord-light");
		expect(document.documentElement.style.colorScheme).toBe("light");
	});

	it("resolves system preference to concrete theme", () => {
		mockMatchMedia(true);
		applyTheme("system");
		expect(document.documentElement.getAttribute("data-theme")).toBe("agentd-dark");
	});

	it("sets CSS custom properties on root", () => {
		applyTheme("tokyo-night");
		expect(document.documentElement.style.getPropertyValue("--th-page")).toBe("#1a1b26");
	});
});

describe("readPersistedTheme", () => {
	beforeEach(() => {
		localStorage.clear();
	});

	it("returns system when nothing is stored", () => {
		expect(readPersistedTheme()).toBe("system");
	});

	it("migrates legacy light to agentd-light", () => {
		localStorage.setItem(
			"agentd:settings",
			JSON.stringify({ ui: { theme: "light" } }),
		);
		expect(readPersistedTheme()).toBe("agentd-light");
	});

	it("migrates legacy dark to agentd-dark", () => {
		localStorage.setItem(
			"agentd:settings",
			JSON.stringify({ ui: { theme: "dark" } }),
		);
		expect(readPersistedTheme()).toBe("agentd-dark");
	});

	it("returns named theme IDs as-is", () => {
		localStorage.setItem(
			"agentd:settings",
			JSON.stringify({ ui: { theme: "tokyo-night" } }),
		);
		expect(readPersistedTheme()).toBe("tokyo-night");
	});

	it("returns system on malformed JSON", () => {
		localStorage.setItem("agentd:settings", "not json {{{");
		expect(readPersistedTheme()).toBe("system");
	});
});
