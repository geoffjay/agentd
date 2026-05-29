/**
 * Theme registry -- central map of all available themes.
 */

import type { ThemeTokens } from "../theme-tokens";
import { agentdDark } from "./agentd-dark";
import { agentdLight } from "./agentd-light";
import { catppuccinLatte } from "./catppuccin-latte";
import { catppuccinMocha } from "./catppuccin-mocha";
import { kanagawa } from "./kanagawa";
import { nordDark } from "./nord-dark";
import { nordLight } from "./nord-light";
import { tokyoNight } from "./tokyo-night";

/** All registered themes keyed by ID. */
export const THEME_REGISTRY: Record<string, ThemeTokens> = {
	"agentd-dark": agentdDark,
	"agentd-light": agentdLight,
	"tokyo-night": tokyoNight,
	kanagawa: kanagawa,
	"nord-dark": nordDark,
	"nord-light": nordLight,
	"catppuccin-mocha": catppuccinMocha,
	"catppuccin-latte": catppuccinLatte,
};

/** Default theme when nothing is persisted. */
export const DEFAULT_THEME_ID = "agentd-dark";

/** Default light/dark IDs used when resolving "system" preference. */
export const SYSTEM_DARK_ID = "agentd-dark";
export const SYSTEM_LIGHT_ID = "agentd-light";

/** Set of theme IDs that belong to the "dark" family. */
export const DARK_THEME_IDS = new Set(
	Object.values(THEME_REGISTRY)
		.filter((t) => t.family === "dark")
		.map((t) => t.id),
);

/** Ordered list for the theme picker UI. */
export const THEME_LIST: Array<{
	id: string;
	name: string;
	family: "dark" | "light";
}> = [
	{ id: "agentd-dark", name: "agentd Dark", family: "dark" },
	{ id: "agentd-light", name: "agentd Light", family: "light" },
	{ id: "tokyo-night", name: "Tokyo Night", family: "dark" },
	{ id: "kanagawa", name: "Kanagawa", family: "dark" },
	{ id: "nord-dark", name: "Nord", family: "dark" },
	{ id: "nord-light", name: "Nord Light", family: "light" },
	{ id: "catppuccin-mocha", name: "Catppuccin Mocha", family: "dark" },
	{ id: "catppuccin-latte", name: "Catppuccin Latte", family: "light" },
];

/**
 * Critical background colors for the anti-FOUC inline script.
 * Must be kept in sync with the theme definitions.
 */
export const CRITICAL_BG: Record<string, string> = Object.fromEntries(
	Object.values(THEME_REGISTRY).map((t) => [t.id, t.page]),
);
