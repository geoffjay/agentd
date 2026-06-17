/**
 * Client for the Knowledge service.
 *
 * Requests are routed through the core service's API gateway
 * (`/api/v1/knowledge/*`) rather than hitting the knowledge service directly:
 * the browser only ever talks to core, which proxies to the knowledge service
 * server-side and injects the tenant context. The method paths below are the
 * knowledge service's own routes; the gateway prefix lives in the base URL.
 *
 * Provides strongly-typed methods for all document operations including
 * creating, listing, updating, deleting, and retrieving the virtual tree.
 */

import type { HealthResponse, PaginatedResponse } from "@/types/common";
import type {
	CreateDocumentRequest,
	Document,
	DocumentContent,
	DocumentListParams,
	TreeNode,
	UpdateDocumentRequest,
} from "@/types/knowledge";
import { ApiClient, withAuth } from "./base";
import { serviceConfig } from "./config";

export class KnowledgeClient extends ApiClient {
	// -------------------------------------------------------------------------
	// Health
	// -------------------------------------------------------------------------

	/** `GET /health` — service health check. */
	getHealth(): Promise<HealthResponse> {
		return this.get<HealthResponse>("/health");
	}

	// -------------------------------------------------------------------------
	// Documents – collection
	// -------------------------------------------------------------------------

	/** `GET /projects/:projectId/documents` — list documents with optional filters. */
	listDocuments(
		projectId: string,
		params?: DocumentListParams,
	): Promise<PaginatedResponse<Document>> {
		return this.get<PaginatedResponse<Document>>(
			`/projects/${projectId}/documents`,
			params as Record<string, string | number | boolean | undefined>,
		);
	}

	/** `POST /projects/:projectId/documents` — create a new document. */
	createDocument(
		projectId: string,
		request: CreateDocumentRequest,
	): Promise<Document> {
		return this.post<Document>(`/projects/${projectId}/documents`, request);
	}

	/** `DELETE /projects/:projectId/documents` — bulk-delete all documents. */
	bulkDeleteDocuments(projectId: string): Promise<void> {
		return this.delete<void>(`/projects/${projectId}/documents`);
	}

	// -------------------------------------------------------------------------
	// Documents – instance
	// -------------------------------------------------------------------------

	/** `GET /projects/:projectId/documents/:docId` — get document metadata. */
	getDocument(projectId: string, docId: string): Promise<Document> {
		return this.get<Document>(`/projects/${projectId}/documents/${docId}`);
	}

	/** `GET /projects/:projectId/documents/:docId/content` — get metadata + body. */
	getDocumentContent(
		projectId: string,
		docId: string,
	): Promise<DocumentContent> {
		return this.get<DocumentContent>(
			`/projects/${projectId}/documents/${docId}/content`,
		);
	}

	/** `PUT /projects/:projectId/documents/:docId` — update a document. */
	updateDocument(
		projectId: string,
		docId: string,
		request: UpdateDocumentRequest,
	): Promise<Document> {
		return this.put<Document>(
			`/projects/${projectId}/documents/${docId}`,
			request,
		);
	}

	/** `DELETE /projects/:projectId/documents/:docId` — delete a document. */
	deleteDocument(projectId: string, docId: string): Promise<void> {
		return this.delete<void>(`/projects/${projectId}/documents/${docId}`);
	}

	// -------------------------------------------------------------------------
	// Tree
	// -------------------------------------------------------------------------

	/** `GET /projects/:projectId/tree` — get the virtual folder/file tree. */
	getTree(projectId: string): Promise<TreeNode[]> {
		return this.get<TreeNode[]>(`/projects/${projectId}/tree`);
	}
}

/**
 * Singleton client routed through the core API gateway. Core proxies
 * `/api/v1/knowledge/*` to the knowledge service and injects `X-Tenant-ID`,
 * so the bearer token is attached via `withAuth`.
 */
export const knowledgeClient = new KnowledgeClient(
	withAuth({ baseUrl: `${serviceConfig.coreServiceUrl}/api/v1/knowledge` }),
);
