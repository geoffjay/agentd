import { Navigate, Outlet } from "react-router-dom";
import { useAuthStore } from "@/stores/authStore";

/**
 * Route guard — redirects unauthenticated users to /login.
 *
 * Usage:
 *   <Route element={<RequireAuth />}>
 *     <Route element={<AppShell />}>
 *       ...protected routes...
 *     </Route>
 *   </Route>
 */
export function RequireAuth() {
	const { isAuthenticated } = useAuthStore();
	if (!isAuthenticated) {
		return <Navigate to="/login" replace />;
	}
	return <Outlet />;
}
