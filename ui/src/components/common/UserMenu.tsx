/**
 * UserMenu — header avatar button with a dropdown for the signed-in user.
 *
 * Replaces the standalone settings icon: shows the current user (name, email,
 * and a superuser badge when applicable), relocates the Settings link into the
 * menu, and adds a Log out action. Logout clears the session server-side
 * (`authApi.logout`) and locally (`authStore.logout`), then routes to /login.
 */

import { LogOut, Settings, User as UserIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { authApi } from "@/services/auth";
import { useAuthStore } from "@/stores/authStore";

/** Derive up-to-two-letter initials from a display name or username. */
function initials(name: string): string {
	const parts = name.trim().split(/\s+/).filter(Boolean);
	if (parts.length === 0) return "";
	const first = parts[0] ?? "";
	const last = parts.length > 1 ? (parts[parts.length - 1] ?? "") : "";
	return (first.charAt(0) + last.charAt(0)).toUpperCase();
}

export function UserMenu() {
	const { user, logout } = useAuthStore();
	const navigate = useNavigate();
	const [open, setOpen] = useState(false);
	const containerRef = useRef<HTMLDivElement>(null);

	// Close on outside click or Escape while open.
	useEffect(() => {
		if (!open) return;
		function onPointerDown(e: MouseEvent) {
			if (
				containerRef.current &&
				!containerRef.current.contains(e.target as Node)
			) {
				setOpen(false);
			}
		}
		function onKeyDown(e: KeyboardEvent) {
			if (e.key === "Escape") setOpen(false);
		}
		document.addEventListener("mousedown", onPointerDown);
		document.addEventListener("keydown", onKeyDown);
		return () => {
			document.removeEventListener("mousedown", onPointerDown);
			document.removeEventListener("keydown", onKeyDown);
		};
	}, [open]);

	const label =
		user?.display_name || user?.username || user?.email || "Account";
	const avatarText = initials(user?.display_name || user?.username || "");

	async function handleLogout() {
		setOpen(false);
		try {
			await authApi.logout();
		} finally {
			logout();
			navigate("/login", { replace: true });
		}
	}

	return (
		<div ref={containerRef} className="relative">
			<button
				type="button"
				aria-label="User menu"
				aria-haspopup="menu"
				aria-expanded={open}
				onClick={() => setOpen((o) => !o)}
				className="flex h-9 w-9 items-center justify-center rounded-full border border-th-border-strong bg-th-surface text-sm font-semibold text-th-text-secondary transition-colors hover:bg-th-surface-hover hover:text-th-text"
			>
				{avatarText ? (
					<span>{avatarText}</span>
				) : (
					<UserIcon size={18} aria-hidden="true" />
				)}
			</button>

			{open && (
				<div className="absolute right-0 mt-2 w-56 overflow-hidden rounded-md border border-th-border bg-th-surface shadow-lg">
					<div className="border-b border-th-border px-4 py-3">
						<p className="truncate text-sm font-medium text-th-text">{label}</p>
						{user?.email && (
							<p className="truncate text-xs text-th-text-muted">
								{user.email}
							</p>
						)}
						{user?.is_superuser && (
							<span className="mt-1 inline-block rounded-full bg-th-accent/10 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-th-accent">
								superuser
							</span>
						)}
					</div>

					<div className="py-1">
						<Link
							to="/settings"
							onClick={() => setOpen(false)}
							className="flex items-center gap-2 px-4 py-2 text-sm text-th-text-secondary hover:bg-th-surface-hover hover:text-th-text"
						>
							<Settings size={16} aria-hidden="true" />
							Settings
						</Link>
						<button
							type="button"
							onClick={handleLogout}
							className="flex w-full items-center gap-2 px-4 py-2 text-left text-sm text-th-text-secondary hover:bg-th-surface-hover hover:text-th-text"
						>
							<LogOut size={16} aria-hidden="true" />
							Log out
						</button>
					</div>
				</div>
			)}
		</div>
	);
}

export default UserMenu;
