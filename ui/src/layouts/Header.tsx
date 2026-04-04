/**
 * Header — fixed top bar with sidebar toggle, search, theme toggle,
 * connection status, notifications, and settings.
 *
 * Positioned to the right of the sidebar. Logo/branding lives in the Sidebar.
 * The search button opens the global SearchPalette (managed by AppShell).
 * Ctrl+K / Cmd+K is handled at the AppShell level.
 */

import { Bell, Menu, Search, Settings } from "lucide-react";
import { Link } from "react-router-dom";
import { ConnectionStatus } from "@/components/common/ConnectionStatus";
import { ThemeToggle } from "@/components/common/ThemeToggle";
import { useAllAgentsStream } from "@/hooks/useAllAgentsStream";
import { useNotificationCount } from "@/hooks/useNotificationCount";
import { useLayout } from "./context";

// ---------------------------------------------------------------------------
// Notification badge
// ---------------------------------------------------------------------------

interface NotificationBadgeProps {
	count: number;
}

function NotificationBadge({ count }: NotificationBadgeProps) {
	if (count === 0) return null;
	return (
		<span
			aria-label={`${count} unread notifications`}
			className="absolute -right-1 -top-1 flex h-4 w-4 items-center justify-center rounded-full bg-th-status-error-dot text-[10px] font-bold text-th-text-inverse"
		>
			{count > 99 ? "99+" : count}
		</span>
	);
}

// ---------------------------------------------------------------------------
// Search trigger button
// ---------------------------------------------------------------------------

function SearchTrigger() {
	const { openSearch } = useLayout();

	return (
		<button
			type="button"
			aria-label="Global search"
			aria-keyshortcuts="Control+k Meta+k"
			onClick={openSearch}
			className="flex items-center gap-2 rounded-md border border-th-border-strong bg-th-surface py-1.5 pl-3 pr-4 text-sm text-th-text-muted transition-colors hover:border-th-border-strong hover:text-th-text-secondary"
		>
			<Search size={14} aria-hidden="true" />
			<span className="hidden md:inline">Search…</span>
			<kbd className="hidden rounded border border-th-border-strong px-1 py-0.5 text-[10px] text-th-text-muted md:inline">
				Ctrl+K
			</kbd>
		</button>
	);
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

export interface HeaderProps {
	/** Number of unread notifications to show in the badge; if omitted, fetched automatically */
	unreadCount?: number;
}

export function Header({ unreadCount }: HeaderProps) {
	const { sidebarOpen, toggleSidebar } = useLayout();
	const { connectionState } = useAllAgentsStream();
	const { pending } = useNotificationCount({ refreshInterval: 15_000 });
	const displayCount = unreadCount ?? pending;

	return (
		<header
			className={[
				"fixed top-2 right-2 z-30 flex h-16 items-center gap-3 px-4 transition-all duration-300 ease-in-out rounded-t-lg",
				"border border-th-border",
				"bg-th-surface-sunken",
				// 'shadow-xl',
				// 'backdrop-blur-md',
				// Offset left edge by sidebar width
				sidebarOpen ? "lg:left-60" : "lg:left-16",
				"left-0",
			].join(" ")}
		>
			{/* Sidebar toggle */}
			<button
				type="button"
				aria-label="Toggle sidebar"
				onClick={toggleSidebar}
				className="rounded-md p-2 text-th-text-muted transition-colors hover:bg-th-surface-hover hover:text-th-text"
			>
				<Menu size={20} />
			</button>

			{/* Search trigger */}
			<SearchTrigger />

			{/* Spacer */}
			<div className="flex-1" />

			{/* Global stream connection status (icon only on small screens) */}
			<ConnectionStatus
				connectionState={connectionState}
				iconOnly
				className="hidden sm:flex"
			/>

			{/* Theme toggle */}
			<ThemeToggle />

			{/* Notification bell */}
			<Link
				to="/notifications"
				aria-label={
					displayCount > 0
						? `Notifications — ${displayCount} unread`
						: "Notifications"
				}
				className="relative rounded-md p-2 text-th-text-muted transition-colors hover:bg-th-surface-hover hover:text-th-text"
			>
				<Bell size={20} />
				<NotificationBadge count={displayCount} />
			</Link>

			{/* Settings */}
			<Link
				to="/settings"
				aria-label="Settings"
				className="rounded-md p-2 text-th-text-muted transition-colors hover:bg-th-surface-hover hover:text-th-text"
			>
				<Settings size={20} />
			</Link>
		</header>
	);
}

export default Header;
