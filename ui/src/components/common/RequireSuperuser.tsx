import { Navigate, Outlet } from "react-router-dom";
import { useAuthStore } from "@/stores/authStore";

/**
 * Route guard — restricts nested routes to product-level superusers.
 *
 * Must be nested **inside** `RequireAuth` (which triggers `checkSession()` to
 * populate the current user). While the session is still being checked we render
 * a lightweight loading state rather than redirecting, to avoid a false-negative
 * redirect before the user/role is known. Non-superusers are sent to the
 * dashboard.
 *
 * This is a convenience/UX gate only — the core service independently enforces
 * superuser access on every `/api/v1/admin/*` endpoint.
 */
export function RequireSuperuser() {
	const { user, sessionChecked } = useAuthStore();

	if (!sessionChecked) {
		return (
			<div className="p-8 text-sm text-th-text-muted" aria-live="polite">
				Checking access…
			</div>
		);
	}

	if (!user?.is_superuser) {
		return <Navigate to="/" replace />;
	}

	return <Outlet />;
}
