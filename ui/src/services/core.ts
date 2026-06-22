/**
 * Client for the Core service (default port 17000).
 *
 * The core service is the API gateway and owns the canonical project entity.
 * Project CRUD operations are served from `/api/v1/projects`.
 */

import type { PaginatedResponse } from "@/types/common";
import type { ListProjectsParams, Project } from "@/types/orchestrator";
import { ApiClient, withAuth } from "./base";
import { serviceConfig } from "./config";

export class CoreClient extends ApiClient {
	// -------------------------------------------------------------------------
	// Projects
	// -------------------------------------------------------------------------

	/** `GET /api/v1/projects` — list all projects. */
	listProjects(
		params?: ListProjectsParams,
	): Promise<PaginatedResponse<Project>> {
		return this.get<PaginatedResponse<Project>>(
			"/api/v1/projects",
			params as Record<string, string | number | boolean | undefined>,
		);
	}
}

/** Singleton client instance using the configured core service URL */
export const coreClient = new CoreClient(
	withAuth({ baseUrl: serviceConfig.coreServiceUrl }),
);
