/**
 * Client for the Knowledge service (default port 17011).
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
import { ApiClient } from "./base";
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

/** Singleton client instance using the configured service URL. */
export const knowledgeClient = new KnowledgeClient({
	baseUrl: serviceConfig.knowledgeServiceUrl,
});
