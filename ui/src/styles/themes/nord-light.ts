import type { ThemeTokens } from "../theme-tokens";

export const nordLight: ThemeTokens = {
	id: "nord-light",
	name: "Nord Light",
	family: "light",

	// Surfaces
	page: "#eceff4",
	pageInset: "#e5e9f0",
	surface: "#ffffff",
	surfaceRaised: "#eceff4",
	surfaceHover: "#e5e9f0",
	surfaceSunken: "#e5e9f0",
	overlay: "rgba(0, 0, 0, 0.40)",
	nav: "#2e3440",
	navHover: "#434c5e",
	navActive: "#5e81ac",
	input: "#ffffff",

	// Text
	text: "#2e3440",
	textSecondary: "#3b4252",
	textMuted: "#4c566a",
	textFaint: "#7b8394",
	textInverse: "#eceff4",
	textNav: "#81899b",
	textNavActive: "#eceff4",
	textLink: "#5e81ac",

	// Borders
	border: "#d8dee9",
	borderStrong: "#c2cad6",
	borderSubtle: "#e5e9f0",
	borderNav: "#4c566a",
	borderInput: "#c2cad6",
	borderFocus: "#5e81ac",

	// Accent
	accent: "#5e81ac",
	accentHover: "#4f7099",
	accentSubtle: "rgba(94, 129, 172, 0.10)",
	accentText: "#ffffff",
	accentSecondary: "#81a1c1",

	// Status -- success
	statusSuccessBg: "rgba(163, 190, 140, 0.15)",
	statusSuccessText: "#5a7744",
	statusSuccessDot: "#a3be8c",
	statusSuccessBorder: "rgba(163, 190, 140, 0.35)",

	// Status -- warning
	statusWarningBg: "rgba(235, 203, 139, 0.18)",
	statusWarningText: "#8a6d20",
	statusWarningDot: "#d4a840",
	statusWarningBorder: "rgba(235, 203, 139, 0.40)",

	// Status -- error
	statusErrorBg: "rgba(191, 97, 106, 0.12)",
	statusErrorText: "#a14048",
	statusErrorDot: "#bf616a",
	statusErrorBorder: "rgba(191, 97, 106, 0.30)",

	// Status -- info
	statusInfoBg: "rgba(136, 192, 208, 0.12)",
	statusInfoText: "#3b7d93",
	statusInfoDot: "#88c0d0",
	statusInfoBorder: "rgba(136, 192, 208, 0.30)",

	// Misc
	focusRing: "#5e81ac",
	focusRingOffset: "#ffffff",
	codeBg: "#e5e9f0",
};
