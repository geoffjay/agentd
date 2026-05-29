import type { ThemeTokens } from "../theme-tokens";

export const tokyoNight: ThemeTokens = {
	id: "tokyo-night",
	name: "Tokyo Night",
	family: "dark",

	// Surfaces
	page: "#1a1b26",
	pageInset: "#16161e",
	surface: "#1f2335",
	surfaceRaised: "#292e42",
	surfaceHover: "#292e42",
	surfaceSunken: "#16161e",
	overlay: "rgba(0, 0, 0, 0.55)",
	nav: "#16161e",
	navHover: "#292e42",
	navActive: "#3d59a1",
	input: "#1f2335",

	// Text
	text: "#c0caf5",
	textSecondary: "#a9b1d6",
	textMuted: "#565f89",
	textFaint: "#3b4261",
	textInverse: "#1a1b26",
	textNav: "#565f89",
	textNavActive: "#c0caf5",
	textLink: "#7aa2f7",

	// Borders
	border: "#292e42",
	borderStrong: "#3b4261",
	borderSubtle: "#1f2335",
	borderNav: "#3b4261",
	borderInput: "#3b4261",
	borderFocus: "#7aa2f7",

	// Accent
	accent: "#7aa2f7",
	accentHover: "#5d8cf5",
	accentSubtle: "rgba(122, 162, 247, 0.15)",
	accentText: "#1a1b26",
	accentSecondary: "#bb9af7",

	// Status -- success
	statusSuccessBg: "rgba(158, 206, 106, 0.15)",
	statusSuccessText: "#9ece6a",
	statusSuccessDot: "#9ece6a",
	statusSuccessBorder: "rgba(158, 206, 106, 0.30)",

	// Status -- warning
	statusWarningBg: "rgba(224, 175, 104, 0.15)",
	statusWarningText: "#e0af68",
	statusWarningDot: "#e0af68",
	statusWarningBorder: "rgba(224, 175, 104, 0.30)",

	// Status -- error
	statusErrorBg: "rgba(247, 118, 142, 0.15)",
	statusErrorText: "#f7768e",
	statusErrorDot: "#f7768e",
	statusErrorBorder: "rgba(247, 118, 142, 0.30)",

	// Status -- info
	statusInfoBg: "rgba(125, 207, 255, 0.15)",
	statusInfoText: "#7dcfff",
	statusInfoDot: "#7dcfff",
	statusInfoBorder: "rgba(125, 207, 255, 0.30)",

	// Misc
	focusRing: "#7aa2f7",
	focusRingOffset: "#1a1b26",
	codeBg: "#16161e",
};
