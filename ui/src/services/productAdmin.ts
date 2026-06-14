/**
 * Client for the core service product-admin endpoints (`/api/v1/admin/*`).
 *
 * These are superuser-only, product-wide (not tenant-scoped) read-only views.
 * The backend enforces access via the `SuperUser` extractor — this client just
 * attaches the bearer token. A non-superuser receives HTTP 403.
 */

import type {
	AdminMembership,
	AdminOrganization,
	AdminSession,
	AdminUser,
} from "@/types/admin";
import type { PaginatedResponse, PaginationParams } from "@/types/common";
import { ApiClient, withAuth } from "./base";
import { serviceConfig } from "./config";

type Query = Record<string, string | number | boolean | undefined>;

export class ProductAdminClient extends ApiClient {
	/** `GET /api/v1/admin/users` — all users across the product. */
	listUsers(params?: PaginationParams): Promise<PaginatedResponse<AdminUser>> {
		return this.get<PaginatedResponse<AdminUser>>(
			"/api/v1/admin/users",
			params as Query,
		);
	}

	/** `GET /api/v1/admin/organizations` — all organizations across the product. */
	listOrganizations(
		params?: PaginationParams,
	): Promise<PaginatedResponse<AdminOrganization>> {
		return this.get<PaginatedResponse<AdminOrganization>>(
			"/api/v1/admin/organizations",
			params as Query,
		);
	}

	/** `GET /api/v1/admin/memberships` — all memberships across every organization. */
	listMemberships(
		params?: PaginationParams,
	): Promise<PaginatedResponse<AdminMembership>> {
		return this.get<PaginatedResponse<AdminMembership>>(
			"/api/v1/admin/memberships",
			params as Query,
		);
	}

	/** `GET /api/v1/admin/sessions` — all sessions (token values never exposed). */
	listSessions(
		params?: PaginationParams,
	): Promise<PaginatedResponse<AdminSession>> {
		return this.get<PaginatedResponse<AdminSession>>(
			"/api/v1/admin/sessions",
			params as Query,
		);
	}
}

/** Singleton client pointing at the core service with bearer-token auth. */
export const productAdminClient = new ProductAdminClient(
	withAuth({ baseUrl: serviceConfig.coreServiceUrl }),
);
