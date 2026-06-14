import { Navigate, Outlet, useLocation } from "react-router-dom";
import { useAuthStore } from "@/stores/authStore";

/**
 * Route guard — redirects unauthenticated users to /login.
 *
 * The current location is passed as `state.from` so that LoginPage can
 * redirect the user back to the page they originally requested after a
 * successful login.
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
	const location = useLocation();
	if (!isAuthenticated) {
		return <Navigate to="/login" state={{ from: location }} replace />;
	}
	return <Outlet />;
}
