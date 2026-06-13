import { create } from "zustand";

const TOKEN_KEY = "agentd_token";

interface AuthState {
	token: string | null;
	isAuthenticated: boolean;
	login: (token: string) => void;
	logout: () => void;
}

export const useAuthStore = create<AuthState>((set) => ({
	token: localStorage.getItem(TOKEN_KEY),
	isAuthenticated: !!localStorage.getItem(TOKEN_KEY),
	login: (token: string) => {
		localStorage.setItem(TOKEN_KEY, token);
		set({ token, isAuthenticated: true });
	},
	logout: () => {
		localStorage.removeItem(TOKEN_KEY);
		set({ token: null, isAuthenticated: false });
	},
}));

export function getStoredToken(): string | null {
	return localStorage.getItem(TOKEN_KEY);
}
