/**
 * useNivoTheme / buildNivoTheme -- unit tests.
 */

import { describe, expect, it } from "vitest";
import { buildNivoTheme } from "@/hooks/useNivoTheme";
import type { ThemeTokens } from "@/styles/theme-tokens";

const MOCK_TOKENS: ThemeTokens = {
	surface: "#1a1a1a",
	surfaceHover: "#222",
	surfaceRaised: "#2a2a2a",
	surfaceSunken: "#111",
	text: "#fff",
	textSecondary: "#ccc",
	textMuted: "#888",
	textFaint: "#666",
	textLink: "#6af",
	border: "#333",
	borderStrong: "#555",
	borderInput: "#444",
	accent: "#3b82f6",
	accentHover: "#2563eb",
	accentSubtle: "#1e3a5f",
	accentText: "#fff",
	focusRing: "#3b82f6",
	focusRingOffset: "#1a1a1a",
	input: "#222",
	statusSuccessBg: "#052e16",
	statusSuccessBorder: "#166534",
	statusSuccessText: "#4ade80",
	statusSuccessDot: "#22c55e",
	statusWarningBg: "#422006",
	statusWarningBorder: "#854d0e",
	statusWarningText: "#facc15",
	statusWarningDot: "#eab308",
	statusErrorBg: "#450a0a",
	statusErrorBorder: "#991b1b",
	statusErrorText: "#f87171",
	statusErrorDot: "#ef4444",
} as ThemeTokens;

describe("buildNivoTheme", () => {
	it("sets background from surface token", () => {
		const theme = buildNivoTheme(MOCK_TOKENS);
		expect(theme.background).toBe("#1a1a1a");
	});

	it("uses textMuted for axis tick text", () => {
		const theme = buildNivoTheme(MOCK_TOKENS);
		expect(theme.axis?.ticks?.text?.fill).toBe("#888");
	});

	it("uses border for grid lines", () => {
		const theme = buildNivoTheme(MOCK_TOKENS);
		expect(theme.grid?.line?.stroke).toBe("#333");
	});

	it("uses surfaceRaised for tooltip background", () => {
		const theme = buildNivoTheme(MOCK_TOKENS);
		expect(theme.tooltip?.container?.background).toBe("#2a2a2a");
	});

	it("uses textSecondary for legend text", () => {
		const theme = buildNivoTheme(MOCK_TOKENS);
		expect(theme.legends?.text?.fill).toBe("#ccc");
	});

	it("uses borderStrong for axis domain line", () => {
		const theme = buildNivoTheme(MOCK_TOKENS);
		expect(theme.axis?.domain?.line?.stroke).toBe("#555");
	});
});
