/**
 * Client for the Core service auth endpoints (default port 17000).
 *
 * Provides login, register, and logout operations. Intentionally does NOT use
 * `withAuth()` — injecting a bearer token on the login/register calls would
 * create a circular dependency (there is no valid token yet).
 */

import { TOKEN_KEY } from "@/stores/authStore";
import { ApiClient } from "./base";
import { serviceConfig } from "./config";

export interface LoginRequest {
	username: string;
	password: string;
}

export interface RegisterRequest {
	username: string;
	email: string;
	password: string;
}

export interface AuthResponse {
	token: string;
	user_id: string;
}

class AuthApiClient extends ApiClient {
	constructor() {
		super({ baseUrl: serviceConfig.coreServiceUrl });
	}

	login(req: LoginRequest): Promise<AuthResponse> {
		return this.post<AuthResponse>("/auth/login", req);
	}

	register(req: RegisterRequest): Promise<AuthResponse> {
		return this.post<AuthResponse>("/auth/register", req);
	}

	async logout(): Promise<void> {
		const token = localStorage.getItem(TOKEN_KEY);
		if (token) {
			try {
				await fetch(`${serviceConfig.coreServiceUrl}/auth/logout`, {
					method: "POST",
					headers: {
						Authorization: `Bearer ${token}`,
						"Content-Type": "application/json",
					},
				});
			} catch {
				// ignore errors on logout — local state is cleared regardless
			}
		}
	}
}

/** Singleton client instance for auth operations */
export const authApi = new AuthApiClient();
