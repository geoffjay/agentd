import type { ThemeTokens } from "../theme-tokens";

export const nordDark: ThemeTokens = {
  id: "nord-dark",
  name: "Nord",
  family: "dark",

  // Surfaces
  page: "#2e3440",
  pageInset: "#272c36",
  surface: "#3b4252",
  surfaceRaised: "#434c5e",
  surfaceHover: "#434c5e",
  surfaceSunken: "#272c36",
  overlay: "rgba(0, 0, 0, 0.50)",
  nav: "#272c36",
  navHover: "#434c5e",
  navActive: "#5e81ac",
  input: "#3b4252",

  // Text
  text: "#eceff4",
  textSecondary: "#d8dee9",
  textMuted: "#81899b",
  textFaint: "#4c566a",
  textInverse: "#2e3440",
  textNav: "#81899b",
  textNavActive: "#eceff4",
  textLink: "#88c0d0",

  // Borders
  border: "#434c5e",
  borderStrong: "#4c566a",
  borderSubtle: "#3b4252",
  borderNav: "#4c566a",
  borderInput: "#4c566a",
  borderFocus: "#88c0d0",

  // Accent
  accent: "#88c0d0",
  accentHover: "#81a1c1",
  accentSubtle: "rgba(136, 192, 208, 0.15)",
  accentText: "#2e3440",
  accentSecondary: "#5e81ac",

  // Status -- success
  statusSuccessBg: "rgba(163, 190, 140, 0.15)",
  statusSuccessText: "#a3be8c",
  statusSuccessDot: "#a3be8c",
  statusSuccessBorder: "rgba(163, 190, 140, 0.30)",

  // Status -- warning
  statusWarningBg: "rgba(235, 203, 139, 0.15)",
  statusWarningText: "#ebcb8b",
  statusWarningDot: "#ebcb8b",
  statusWarningBorder: "rgba(235, 203, 139, 0.30)",

  // Status -- error
  statusErrorBg: "rgba(191, 97, 106, 0.15)",
  statusErrorText: "#bf616a",
  statusErrorDot: "#bf616a",
  statusErrorBorder: "rgba(191, 97, 106, 0.30)",

  // Status -- info
  statusInfoBg: "rgba(136, 192, 208, 0.15)",
  statusInfoText: "#88c0d0",
  statusInfoDot: "#88c0d0",
  statusInfoBorder: "rgba(136, 192, 208, 0.30)",

  // Misc
  focusRing: "#88c0d0",
  focusRingOffset: "#2e3440",
  codeBg: "#272c36",
};
