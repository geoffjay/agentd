/**
 * useChartPalette / withAlpha -- unit tests.
 */

import { renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
	buildChartPalette,
	useChartPalette,
	withAlpha,
} from "@/hooks/useChartPalette";
import { getTheme } from "@/stores/themeStore";
import { DEFAULT_THEME_ID } from "@/styles/themes/index";

describe("withAlpha", () => {
	it("converts #rrggbb hex to rgba", () => {
		expect(withAlpha("#8ac926", 0.7)).toBe("rgba(138, 201, 38, 0.7)");
	});

	it("converts #rgb shorthand hex to rgba", () => {
		expect(withAlpha("#fff", 0.5)).toBe("rgba(255, 255, 255, 0.5)");
	});

	it("converts #rrggbbaa hex to rgba (ignoring the original alpha)", () => {
		expect(withAlpha("#ff595e80", 0.25)).toBe("rgba(255, 89, 94, 0.25)");
	});

	it("converts rgb(...) to rgba", () => {
		expect(withAlpha("rgb(10, 20, 30)", 0.25)).toBe("rgba(10, 20, 30, 0.25)");
	});

	it("replaces the alpha of an existing rgba(...) value", () => {
		expect(withAlpha("rgba(126, 156, 216, 0.15)", 0.4)).toBe(
			"rgba(126, 156, 216, 0.4)",
		);
	});

	it("handles surrounding whitespace", () => {
		expect(withAlpha("  #333  ", 0.5)).toBe("rgba(51, 51, 51, 0.5)");
	});

	it("clamps alpha into the [0, 1] range", () => {
		expect(withAlpha("#000", 2)).toBe("rgba(0, 0, 0, 1)");
		expect(withAlpha("#000", -1)).toBe("rgba(0, 0, 0, 0)");
	});

	it("returns unparseable values unchanged", () => {
		expect(withAlpha("var(--th-accent)", 0.5)).toBe("var(--th-accent)");
		expect(withAlpha("not-a-color", 0.5)).toBe("not-a-color");
	});
});

describe("buildChartPalette", () => {
	const tokens = getTheme(DEFAULT_THEME_ID);
	const palette = buildChartPalette(tokens);

	it("maps semantic colors from status dot tokens", () => {
		expect(palette.success).toBe(tokens.statusSuccessDot);
		expect(palette.warning).toBe(tokens.statusWarningDot);
		expect(palette.error).toBe(tokens.statusErrorDot);
		expect(palette.info).toBe(tokens.statusInfoDot);
	});

	it("maps neutral from textMuted and accent from accent", () => {
		expect(palette.neutral).toBe(tokens.textMuted);
		expect(palette.accent).toBe(tokens.accent);
	});

	it("derives a categorical series array from theme tokens", () => {
		expect(palette.series.length).toBeGreaterThanOrEqual(5);
		expect(palette.series[0]).toBe(tokens.accent);
	});

	it("exposes the withAlpha helper", () => {
		expect(palette.withAlpha("#fff", 0.5)).toBe("rgba(255, 255, 255, 0.5)");
	});
});

describe("useChartPalette", () => {
	it("falls back to the default theme outside a ThemeProvider", () => {
		const { result } = renderHook(() => useChartPalette());
		const tokens = getTheme(DEFAULT_THEME_ID);
		expect(result.current.accent).toBe(tokens.accent);
		expect(result.current.surface).toBe(tokens.surface);
	});
});
