/**
 * Client for the Core service auth endpoints (default port 17000).
 *
 * Provides login, register, and logout operations.
 */

import { runtimeServiceUrl } from "../runtime-config";
import { ApiClient } from "./base";

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

const coreServiceUrl =
	runtimeServiceUrl("core") ??
	import.meta.env.VITE_AGENTD_CORE_SERVICE_URL ??
	"http://localhost:17000";

class AuthApiClient extends ApiClient {
	constructor() {
		super({ baseUrl: coreServiceUrl });
	}

	login(req: LoginRequest): Promise<AuthResponse> {
		return this.post<AuthResponse>("/auth/login", req);
	}

	register(req: RegisterRequest): Promise<AuthResponse> {
		return this.post<AuthResponse>("/auth/register", req);
	}

	async logout(): Promise<void> {
		const token = localStorage.getItem("agentd_token");
		if (token) {
			try {
				await fetch(`${coreServiceUrl}/auth/logout`, {
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
