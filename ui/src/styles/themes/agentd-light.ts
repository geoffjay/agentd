import type { ThemeTokens } from "../theme-tokens";

export const agentdLight: ThemeTokens = {
	id: "agentd-light",
	name: "agentd Light",
	family: "light",

	// Surfaces
	page: "#f4f6ee",
	pageInset: "#eaeedd",
	surface: "#ffffff",
	surfaceRaised: "#f4f6ee",
	surfaceHover: "#eaeedd",
	surfaceSunken: "#eaeedd",
	overlay: "rgba(0, 0, 0, 0.50)",
	nav: "#1e2211",
	navHover: "#3c4323",
	navActive: "#7e501b",
	input: "#ffffff",

	// Text
	text: "#1e2211",
	textSecondary: "#5a6534",
	textMuted: "#778745",
	textFaint: "#95a857",
	textInverse: "#ffffff",
	textNav: "#aaba78",
	textNavActive: "#f4f6ee",
	textLink: "#a86a24",

	// Borders
	border: "#d5dcbc",
	borderStrong: "#c0cb9a",
	borderSubtle: "#eaeedd",
	borderNav: "#5a6534",
	borderInput: "#c0cb9a",
	borderFocus: "#d2852d",

	// Accent
	accent: "#d2852d",
	accentHover: "#a86a24",
	accentSubtle: "#fbf3ea",
	accentText: "#ffffff",
	accentSecondary: "#7e501b",

	// Status -- success
	statusSuccessBg: "rgba(138, 201, 38, 0.12)",
	statusSuccessText: "#5a8a10",
	statusSuccessDot: "#8ac926",
	statusSuccessBorder: "rgba(138, 201, 38, 0.30)",

	// Status -- warning
	statusWarningBg: "rgba(255, 202, 58, 0.12)",
	statusWarningText: "#956d00",
	statusWarningDot: "#e0a800",
	statusWarningBorder: "rgba(255, 202, 58, 0.30)",

	// Status -- error
	statusErrorBg: "rgba(255, 89, 94, 0.10)",
	statusErrorText: "#c4363a",
	statusErrorDot: "#ff595e",
	statusErrorBorder: "rgba(255, 89, 94, 0.25)",

	// Status -- info
	statusInfoBg: "rgba(25, 130, 196, 0.10)",
	statusInfoText: "#146a9e",
	statusInfoDot: "#1982c4",
	statusInfoBorder: "rgba(25, 130, 196, 0.25)",

	// Misc
	focusRing: "#d2852d",
	focusRingOffset: "#ffffff",
	codeBg: "#eaeedd",
};
