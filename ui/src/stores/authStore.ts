import { create } from "zustand";
import { authApi, type User } from "@/services/auth";

/** localStorage key used to persist the session token. Shared across modules. */
export const TOKEN_KEY = "agentd_token";

interface AuthState {
	token: string | null;
	/** Current user, populated on login and on `checkSession()`. */
	user: User | null;
	isAuthenticated: boolean;
	/**
	 * Whether `checkSession()` has completed at least once. Guards prevent a
	 * false-negative redirect (e.g. RequireSuperuser) before the user is loaded.
	 */
	sessionChecked: boolean;
	login: (token: string, user: User | null) => void;
	logout: () => void;
	/** Re-fetch the current user from `/auth/me` using the stored token. */
	checkSession: () => Promise<void>;
}

export const useAuthStore = create<AuthState>((set) => ({
	token: localStorage.getItem(TOKEN_KEY),
	user: null,
	isAuthenticated: !!localStorage.getItem(TOKEN_KEY),
	sessionChecked: false,
	login: (token: string, user: User | null) => {
		localStorage.setItem(TOKEN_KEY, token);
		set({ token, user, isAuthenticated: true, sessionChecked: true });
	},
	logout: () => {
		localStorage.removeItem(TOKEN_KEY);
		set({
			token: null,
			user: null,
			isAuthenticated: false,
			sessionChecked: true,
		});
	},
	checkSession: async () => {
		const token = localStorage.getItem(TOKEN_KEY);
		if (!token) {
			set({
				token: null,
				user: null,
				isAuthenticated: false,
				sessionChecked: true,
			});
			return;
		}
		try {
			const me = await authApi.me();
			set({
				token,
				user: me.user,
				isAuthenticated: true,
				sessionChecked: true,
			});
		} catch {
			// Invalid/expired token — clear it.
			localStorage.removeItem(TOKEN_KEY);
			set({
				token: null,
				user: null,
				isAuthenticated: false,
				sessionChecked: true,
			});
		}
	},
}));

export function getStoredToken(): string | null {
	return localStorage.getItem(TOKEN_KEY);
}
