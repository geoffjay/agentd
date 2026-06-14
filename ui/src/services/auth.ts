/**
 * Client for the Core service auth endpoints (default port 17000).
 *
 * Provides login, register, and logout operations. Intentionally does NOT use
 * `withAuth()` — injecting a bearer token on the login/register calls would
 * create a circular dependency (there is no valid token yet).
 */

import { TOKEN_KEY } from "@/stores/authStore";
import { ApiError } from "@/types/common";
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

/** Authenticated user as returned by the core service (never includes secrets). */
export interface User {
	id: string;
	username: string | null;
	email: string;
	display_name: string | null;
	role: string;
	/** Product-level superuser flag — gates the `/admin` area. */
	is_superuser: boolean;
	active_organization_id: string | null;
	created_at: string;
	updated_at: string;
}

export interface OrganizationSummary {
	id: string;
	name: string;
	slug: string;
	created_at: string;
	updated_at: string;
}

export interface AuthResponse {
	token: string;
	user: User;
	active_organization: OrganizationSummary | null;
}

export interface MeResponse {
	user: User;
	active_organization: OrganizationSummary | null;
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

	/**
	 * `GET /auth/me` — return the current user + active org for the stored token.
	 *
	 * Uses a manual fetch (rather than `withAuth()`) to avoid a circular import
	 * with the auth store, and so the token is read fresh from localStorage.
	 */
	async me(): Promise<MeResponse> {
		const token = localStorage.getItem(TOKEN_KEY);
		const resp = await fetch(`${serviceConfig.coreServiceUrl}/auth/me`, {
			headers: {
				Authorization: `Bearer ${token ?? ""}`,
				Accept: "application/json",
			},
		});
		if (!resp.ok) {
			throw new ApiError(resp.status, `HTTP ${resp.status}`);
		}
		return (await resp.json()) as MeResponse;
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
