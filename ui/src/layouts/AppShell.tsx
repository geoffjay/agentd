/**
 * AppShell — root layout wrapper.
 *
 * Manages sidebar open/closed state (persisted to localStorage),
 * global search palette visibility, provides LayoutContext and
 * ThemeProvider to all children, and handles keyboard shortcuts:
 *   - Ctrl+B: toggle sidebar
 *   - Ctrl+K / Cmd+K: open search palette
 */

import { useCallback, useEffect, useState } from "react";
import { Outlet } from "react-router-dom";
import { SkipNav } from "@/components/common/SkipNav";
import { ToastContainer } from "@/components/common/ToastContainer";
import { SearchPalette } from "@/components/search/SearchPalette";
import { ThemeProvider } from "@/hooks/useTheme";
import { ContentArea } from "./ContentArea";
import { LayoutContext } from "./context";
import { Header } from "./Header";
import { Sidebar } from "./Sidebar";

const STORAGE_KEY = "agentd:sidebar:open";

function readPersistedSidebarState(): boolean {
	try {
		const stored = localStorage.getItem(STORAGE_KEY);
		if (stored === null) return true; // default: expanded on desktop
		return stored === "true";
	} catch {
		return true;
	}
}

export function AppShell() {
	const [sidebarOpen, setSidebarOpenState] = useState<boolean>(
		readPersistedSidebarState,
	);
	const [searchOpen, setSearchOpen] = useState(false);

	const setSidebarOpen = useCallback((open: boolean) => {
		setSidebarOpenState(open);
		try {
			localStorage.setItem(STORAGE_KEY, String(open));
		} catch {
			// ignore storage errors (e.g. private browsing quota)
		}
	}, []);

	const toggleSidebar = useCallback(() => {
		setSidebarOpen(!sidebarOpen);
	}, [sidebarOpen, setSidebarOpen]);

	const openSearch = useCallback(() => setSearchOpen(true), []);
	const closeSearch = useCallback(() => setSearchOpen(false), []);

	// Ctrl+B to toggle sidebar; Ctrl+K / Cmd+K to open search
	useEffect(() => {
		function handleKeyDown(e: KeyboardEvent) {
			if ((e.ctrlKey || e.metaKey) && e.key === "b") {
				e.preventDefault();
				setSidebarOpen(!sidebarOpen);
			}
			if ((e.ctrlKey || e.metaKey) && e.key === "k") {
				e.preventDefault();
				setSearchOpen(true);
			}
		}
		document.addEventListener("keydown", handleKeyDown);
		return () => document.removeEventListener("keydown", handleKeyDown);
	}, [sidebarOpen, setSidebarOpen]);

	return (
		<ThemeProvider>
			<LayoutContext.Provider
				value={{
					sidebarOpen,
					setSidebarOpen,
					toggleSidebar,
					searchOpen,
					openSearch,
					closeSearch,
				}}
			>
				<div className="min-h-screen pt-2 pr-2 pb-2 bg-gray-900 transition-colors duration-150">
					<SkipNav />
					<Sidebar />
					<div
						className={[
							"max-h-[calc(100vh-1rem)]",
							"overflow-y-auto",
							"bg-gray-100 dark:bg-gray-800",
							"transition-all duration-300 ease-in-out",
							"border border-gray-400 dark:border-gray-600",
							"rounded-xl",
							// On large screens, shift right by sidebar width
							sidebarOpen ? "lg:ml-60" : "lg:ml-16",
						].join(" ")}
					>
						<Header />
						<ContentArea>
							<Outlet />
						</ContentArea>
						<SearchPalette isOpen={searchOpen} onClose={closeSearch} />
						<ToastContainer />
					</div>
				</div>
			</LayoutContext.Provider>
		</ThemeProvider>
	);
}

export default AppShell;
