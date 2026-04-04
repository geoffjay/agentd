/**
 * theme-tokens.ts -- ThemeTokens interface and helpers.
 *
 * Every named theme must provide a value for each token defined here.
 * Tokens are applied as CSS custom properties (--th-*) on <html> and
 * consumed by Tailwind v4 utility classes (bg-th-*, text-th-*, etc.).
 */

export interface ThemeTokens {
  /** Unique theme identifier (e.g. "tokyo-night") */
  id: string;
  /** Display name shown in the theme picker */
  name: string;
  /** Whether this is a "dark" or "light" theme -- drives color-scheme */
  family: "dark" | "light";

  // ---------------------------------------------------------------------------
  // Surfaces
  // ---------------------------------------------------------------------------
  /** Page/app background */
  page: string;
  /** Inset/recessed background (e.g. AppShell outer) */
  pageInset: string;
  /** Card/panel background */
  surface: string;
  /** Elevated surface (modals, dropdowns, tooltips) */
  surfaceRaised: string;
  /** Recessed surface (table headers, subtle wells) */
  surfaceSunken: string;
  /** Surface hover state */
  surfaceHover: string;
  /** Modal/dialog backdrop overlay */
  overlay: string;
  /** Sidebar/nav background */
  nav: string;
  /** Sidebar item hover */
  navHover: string;
  /** Sidebar active item background */
  navActive: string;
  /** Input/select background */
  input: string;

  // ---------------------------------------------------------------------------
  // Text
  // ---------------------------------------------------------------------------
  /** Primary text (headings, body) */
  text: string;
  /** Secondary/supporting text */
  textSecondary: string;
  /** Muted/de-emphasized text */
  textMuted: string;
  /** Very faint text (disabled, placeholders) */
  textFaint: string;
  /** Text on permanently dark surfaces */
  textInverse: string;
  /** Sidebar inactive text */
  textNav: string;
  /** Sidebar active text */
  textNavActive: string;
  /** Link/accent-colored text */
  textLink: string;

  // ---------------------------------------------------------------------------
  // Borders
  // ---------------------------------------------------------------------------
  /** Default card/divider border */
  border: string;
  /** Emphasized border */
  borderStrong: string;
  /** Subtle row dividers */
  borderSubtle: string;
  /** Sidebar border */
  borderNav: string;
  /** Input/select border */
  borderInput: string;
  /** Focus ring border */
  borderFocus: string;

  // ---------------------------------------------------------------------------
  // Accent / brand
  // ---------------------------------------------------------------------------
  /** Primary accent (buttons, active states) */
  accent: string;
  /** Accent hover */
  accentHover: string;
  /** Subtle accent background (selected rows, light badges) */
  accentSubtle: string;
  /** Text on accent background */
  accentText: string;
  /** Secondary accent color */
  accentSecondary: string;

  // ---------------------------------------------------------------------------
  // Status -- success
  // ---------------------------------------------------------------------------
  statusSuccessBg: string;
  statusSuccessText: string;
  statusSuccessDot: string;
  statusSuccessBorder: string;

  // ---------------------------------------------------------------------------
  // Status -- warning
  // ---------------------------------------------------------------------------
  statusWarningBg: string;
  statusWarningText: string;
  statusWarningDot: string;
  statusWarningBorder: string;

  // ---------------------------------------------------------------------------
  // Status -- error
  // ---------------------------------------------------------------------------
  statusErrorBg: string;
  statusErrorText: string;
  statusErrorDot: string;
  statusErrorBorder: string;

  // ---------------------------------------------------------------------------
  // Status -- info
  // ---------------------------------------------------------------------------
  statusInfoBg: string;
  statusInfoText: string;
  statusInfoDot: string;
  statusInfoBorder: string;

  // ---------------------------------------------------------------------------
  // Misc
  // ---------------------------------------------------------------------------
  /** Focus ring color */
  focusRing: string;
  /** Focus ring offset background */
  focusRingOffset: string;
  /** Code block / inline code background */
  codeBg: string;
}

/**
 * Maps ThemeTokens field names to CSS custom property names (--th-*).
 * Used by applyTheme() to set properties on the root element.
 */
export const TOKEN_CSS_MAP: Record<string, string> = {
  page: "--th-page",
  pageInset: "--th-page-inset",
  surface: "--th-surface",
  surfaceRaised: "--th-surface-raised",
  surfaceSunken: "--th-surface-sunken",
  surfaceHover: "--th-surface-hover",
  overlay: "--th-overlay",
  nav: "--th-nav",
  navHover: "--th-nav-hover",
  navActive: "--th-nav-active",
  input: "--th-input",

  text: "--th-text",
  textSecondary: "--th-text-secondary",
  textMuted: "--th-text-muted",
  textFaint: "--th-text-faint",
  textInverse: "--th-text-inverse",
  textNav: "--th-text-nav",
  textNavActive: "--th-text-nav-active",
  textLink: "--th-text-link",

  border: "--th-border",
  borderStrong: "--th-border-strong",
  borderSubtle: "--th-border-subtle",
  borderNav: "--th-border-nav",
  borderInput: "--th-border-input",
  borderFocus: "--th-border-focus",

  accent: "--th-accent",
  accentHover: "--th-accent-hover",
  accentSubtle: "--th-accent-subtle",
  accentText: "--th-accent-text",
  accentSecondary: "--th-accent-secondary",

  statusSuccessBg: "--th-status-success-bg",
  statusSuccessText: "--th-status-success-text",
  statusSuccessDot: "--th-status-success-dot",
  statusSuccessBorder: "--th-status-success-border",

  statusWarningBg: "--th-status-warning-bg",
  statusWarningText: "--th-status-warning-text",
  statusWarningDot: "--th-status-warning-dot",
  statusWarningBorder: "--th-status-warning-border",

  statusErrorBg: "--th-status-error-bg",
  statusErrorText: "--th-status-error-text",
  statusErrorDot: "--th-status-error-dot",
  statusErrorBorder: "--th-status-error-border",

  statusInfoBg: "--th-status-info-bg",
  statusInfoText: "--th-status-info-text",
  statusInfoDot: "--th-status-info-dot",
  statusInfoBorder: "--th-status-info-border",

  focusRing: "--th-focus-ring",
  focusRingOffset: "--th-focus-ring-offset",
  codeBg: "--th-code-bg",
};

/** All token field names that should be applied as CSS vars (excludes id, name, family). */
export const TOKEN_KEYS = Object.keys(TOKEN_CSS_MAP);
