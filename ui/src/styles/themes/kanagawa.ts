import type { ThemeTokens } from "../theme-tokens";

export const kanagawa: ThemeTokens = {
  id: "kanagawa",
  name: "Kanagawa",
  family: "dark",

  // Surfaces
  page: "#1f1f28",
  pageInset: "#16161d",
  surface: "#2a2a37",
  surfaceRaised: "#363646",
  surfaceHover: "#363646",
  surfaceSunken: "#16161d",
  overlay: "rgba(0, 0, 0, 0.55)",
  nav: "#16161d",
  navHover: "#363646",
  navActive: "#6a9589",
  input: "#2a2a37",

  // Text
  text: "#dcd7ba",
  textSecondary: "#c8c093",
  textMuted: "#727169",
  textFaint: "#54546d",
  textInverse: "#1f1f28",
  textNav: "#727169",
  textNavActive: "#dcd7ba",
  textLink: "#7e9cd8",

  // Borders
  border: "#363646",
  borderStrong: "#54546d",
  borderSubtle: "#2a2a37",
  borderNav: "#54546d",
  borderInput: "#54546d",
  borderFocus: "#7e9cd8",

  // Accent
  accent: "#7e9cd8",
  accentHover: "#6a88c4",
  accentSubtle: "rgba(126, 156, 216, 0.15)",
  accentText: "#1f1f28",
  accentSecondary: "#957fb8",

  // Status -- success
  statusSuccessBg: "rgba(118, 148, 106, 0.20)",
  statusSuccessText: "#98bb6c",
  statusSuccessDot: "#76946a",
  statusSuccessBorder: "rgba(118, 148, 106, 0.35)",

  // Status -- warning
  statusWarningBg: "rgba(255, 158, 59, 0.15)",
  statusWarningText: "#ff9e3b",
  statusWarningDot: "#ff9e3b",
  statusWarningBorder: "rgba(255, 158, 59, 0.30)",

  // Status -- error
  statusErrorBg: "rgba(195, 64, 67, 0.20)",
  statusErrorText: "#e82424",
  statusErrorDot: "#c34043",
  statusErrorBorder: "rgba(195, 64, 67, 0.35)",

  // Status -- info
  statusInfoBg: "rgba(127, 180, 202, 0.15)",
  statusInfoText: "#7fb4ca",
  statusInfoDot: "#7fb4ca",
  statusInfoBorder: "rgba(127, 180, 202, 0.30)",

  // Misc
  focusRing: "#7e9cd8",
  focusRingOffset: "#1f1f28",
  codeBg: "#16161d",
};
