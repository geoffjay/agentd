import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { KnowledgeClient } from "@/services/knowledge";
import type { Document, DocumentContent, TreeNode } from "@/types/knowledge";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeJsonResponse(status: number, body: unknown) {
	return new Response(JSON.stringify(body), {
		status,
		headers: new Headers({ "content-type": "application/json" }),
	});
}

function mockFetch(status: number, body: unknown) {
	vi.stubGlobal(
		"fetch",
		vi.fn().mockResolvedValue(makeJsonResponse(status, body)),
	);
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const PROJECT_ID = "550e8400-e29b-41d4-a716-446655440000";
const DOC_ID = "6ba7b810-9dad-11d1-80b4-00c04fd430c8";

const mockDocument: Document = {
	id: DOC_ID,
	project_id: PROJECT_ID,
	organization_id: null,
	rel_path: "docs/readme.md",
	title: "Readme",
	size_bytes: 42,
	created_at: "2026-01-01T00:00:00Z",
	updated_at: "2026-01-01T00:00:00Z",
};

const mockDocumentContent: DocumentContent = {
	document: mockDocument,
	content: "# Readme\n\nHello world.",
};

const mockPage = { items: [mockDocument], total: 1, limit: 50, offset: 0 };

const mockTree: TreeNode[] = [
	{
		type: "folder",
		name: "docs",
		path: "docs/",
		children: [
			{
				type: "file",
				name: "readme.md",
				path: "docs/readme.md",
				doc_id: DOC_ID,
			},
		],
	},
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("KnowledgeClient", () => {
	let client: KnowledgeClient;

	beforeEach(() => {
		client = new KnowledgeClient({
			baseUrl: "http://localhost:17011",
			maxRetries: 1,
		});
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	// -------------------------------------------------------------------------
	// Health
	// -------------------------------------------------------------------------

	it("getHealth returns the health object", async () => {
		mockFetch(200, { service: "knowledge", version: "0.4.11", status: "ok" });
		const h = await client.getHealth();
		expect(h.service).toBe("knowledge");
	});

	// -------------------------------------------------------------------------
	// listDocuments
	// -------------------------------------------------------------------------

	it("listDocuments calls the correct URL", async () => {
		const spy = vi
			.fn()
			.mockResolvedValue(makeJsonResponse(200, mockPage));
		vi.stubGlobal("fetch", spy);

		await client.listDocuments(PROJECT_ID);

		const url: string = spy.mock.calls[0][0];
		expect(url).toContain(`/projects/${PROJECT_ID}/documents`);
	});

	it("listDocuments forwards prefix query param", async () => {
		const spy = vi
			.fn()
			.mockResolvedValue(makeJsonResponse(200, mockPage));
		vi.stubGlobal("fetch", spy);

		await client.listDocuments(PROJECT_ID, { prefix: "docs/" });

		const url: string = spy.mock.calls[0][0];
		expect(url).toContain("prefix=docs%2F");
	});

	it("listDocuments returns paginated documents", async () => {
		mockFetch(200, mockPage);
		const page = await client.listDocuments(PROJECT_ID);
		expect(page.items).toHaveLength(1);
		expect(page.items[0].id).toBe(DOC_ID);
		expect(page.total).toBe(1);
	});

	// -------------------------------------------------------------------------
	// createDocument
	// -------------------------------------------------------------------------

	it("createDocument posts body and returns document", async () => {
		const spy = vi
			.fn()
			.mockResolvedValue(makeJsonResponse(201, mockDocument));
		vi.stubGlobal("fetch", spy);

		const doc = await client.createDocument(PROJECT_ID, {
			rel_path: "docs/readme.md",
			content: "# Hello",
		});

		expect(spy.mock.calls[0][1].method).toBe("POST");
		expect(doc.id).toBe(DOC_ID);
	});

	// -------------------------------------------------------------------------
	// getDocumentContent
	// -------------------------------------------------------------------------

	it("getDocumentContent returns content", async () => {
		mockFetch(200, mockDocumentContent);
		const dc = await client.getDocumentContent(PROJECT_ID, DOC_ID);
		expect(dc.content).toBe("# Readme\n\nHello world.");
		expect(dc.document.rel_path).toBe("docs/readme.md");
	});

	// -------------------------------------------------------------------------
	// updateDocument
	// -------------------------------------------------------------------------

	it("updateDocument sends PUT with body", async () => {
		const spy = vi
			.fn()
			.mockResolvedValue(makeJsonResponse(200, mockDocument));
		vi.stubGlobal("fetch", spy);

		await client.updateDocument(PROJECT_ID, DOC_ID, {
			content: "# Updated",
			expected_updated_at: "2026-01-01T00:00:00Z",
		});

		expect(spy.mock.calls[0][1].method).toBe("PUT");
		const body = JSON.parse(spy.mock.calls[0][1].body as string);
		expect(body.content).toBe("# Updated");
		expect(body.expected_updated_at).toBe("2026-01-01T00:00:00Z");
	});

	// -------------------------------------------------------------------------
	// deleteDocument
	// -------------------------------------------------------------------------

	it("deleteDocument sends DELETE to correct URL", async () => {
		const spy = vi
			.fn()
			.mockResolvedValue(new Response(null, { status: 204 }));
		vi.stubGlobal("fetch", spy);

		await client.deleteDocument(PROJECT_ID, DOC_ID);

		const url: string = spy.mock.calls[0][0];
		expect(url).toContain(`/projects/${PROJECT_ID}/documents/${DOC_ID}`);
		expect(spy.mock.calls[0][1].method).toBe("DELETE");
	});

	// -------------------------------------------------------------------------
	// getTree
	// -------------------------------------------------------------------------

	it("getTree returns tree nodes", async () => {
		mockFetch(200, mockTree);
		const nodes = await client.getTree(PROJECT_ID);
		expect(nodes).toHaveLength(1);
		expect(nodes[0].type).toBe("folder");
		if (nodes[0].type === "folder") {
			expect(nodes[0].children[0].type).toBe("file");
		}
	});

	// -------------------------------------------------------------------------
	// Error handling
	// -------------------------------------------------------------------------

	it("throws ApiError on 404", async () => {
		mockFetch(404, { error: "document not found" });
		await expect(client.getDocument(PROJECT_ID, DOC_ID)).rejects.toMatchObject(
			{ status: 404 },
		);
	});

	it("throws ApiError on 409 conflict", async () => {
		mockFetch(409, { error: "document already exists" });
		await expect(
			client.createDocument(PROJECT_ID, {
				rel_path: "docs/readme.md",
				content: "",
			}),
		).rejects.toMatchObject({ status: 409 });
	});
});
