/**
 * ThemeToggle -- quick theme toggle for the header bar.
 *
 * Cycles between: system -> resolved opposite family -> system
 * Displays an icon representing the current state:
 *   - system -> Monitor icon
 *   - light family -> Sun icon
 *   - dark family -> Moon icon
 */

import { Monitor, Moon, Sun } from "lucide-react";
import { useTheme } from "@/hooks/useTheme";

function ThemeIcon({ family }: { family: "dark" | "light" | "system" }) {
  const size = 18;
  if (family === "dark") return <Moon size={size} aria-hidden="true" />;
  if (family === "light") return <Sun size={size} aria-hidden="true" />;
  return <Monitor size={size} aria-hidden="true" />;
}

export interface ThemeToggleProps {
  className?: string;
}

export function ThemeToggle({ className = "" }: ThemeToggleProps) {
  const { themeId, theme, setTheme } = useTheme();
  const isSystem = themeId === "system";

  function handleClick() {
    if (isSystem) {
      // System -> pick the opposite of what system resolved to
      setTheme(theme.family === "dark" ? "agentd-light" : "agentd-dark");
    } else {
      // Any explicit theme -> back to system
      setTheme("system");
    }
  }

  const display = isSystem ? "system" : theme.family;
  const label = isSystem
    ? `Theme: System (switch to ${theme.family === "dark" ? "Light" : "Dark"})`
    : `Theme: ${theme.name} (switch to System)`;

  return (
    <button
      type="button"
      aria-label={label}
      onClick={handleClick}
      className={[
        "rounded-md p-2 text-th-text-muted transition-colors",
        "hover:bg-th-surface-hover hover:text-th-text",
        "focus:outline-none focus:ring-2 focus:ring-th-focus-ring focus:ring-offset-2 focus:ring-offset-th-focus-ring-offset",
        className,
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <ThemeIcon family={display} />
    </button>
  );
}

export default ThemeToggle;
