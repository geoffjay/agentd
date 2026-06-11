/**
 * AgentFormPage — create and edit flows against the MSW handlers.
 */

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { HttpResponse, http } from "msw";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { AgentFormPage } from "@/pages/agents/AgentFormPage";
import { makeAgent } from "@/test/mocks/factories";
import { server } from "@/test/mocks/server";

const BASE = "http://localhost:17006";

function renderAt(path: string) {
	return render(
		<MemoryRouter initialEntries={[path]}>
			<Routes>
				<Route path="/agents/new" element={<AgentFormPage />} />
				<Route path="/agents/:id/edit" element={<AgentFormPage />} />
				<Route
					path="/agents/:id"
					element={<div data-testid="agent-detail" />}
				/>
				<Route path="/agents" element={<div data-testid="agent-list" />} />
			</Routes>
		</MemoryRouter>,
	);
}

describe("AgentFormPage — create", () => {
	it("renders the create form sections", () => {
		renderAt("/agents/new");
		expect(
			screen.getByRole("heading", { name: "Create Agent" }),
		).toBeInTheDocument();
		expect(screen.getByLabelText(/^name$/i)).toBeInTheDocument();
		expect(screen.getByLabelText(/working directory/i)).toBeInTheDocument();
		expect(screen.getByText("Tool Policy")).toBeInTheDocument();
		expect(screen.getByText("Workspace")).toBeInTheDocument();
	});

	it("shows validation errors instead of submitting an empty form", async () => {
		renderAt("/agents/new");
		fireEvent.click(screen.getByRole("button", { name: /^create agent$/i }));
		expect(await screen.findByText(/name is required/i)).toBeInTheDocument();
		expect(
			screen.getByText(/working directory is required/i),
		).toBeInTheDocument();
	});

	it("POSTs the request and navigates to the new agent's detail page", async () => {
		let posted: Record<string, unknown> | undefined;
		server.use(
			http.post(`${BASE}/agents`, async ({ request }) => {
				posted = (await request.json()) as Record<string, unknown>;
				return HttpResponse.json(
					makeAgent({ id: "created-1", name: String(posted.name) }),
					{ status: 201 },
				);
			}),
		);

		renderAt("/agents/new");
		fireEvent.change(screen.getByLabelText(/^name$/i), {
			target: { value: "my-agent" },
		});
		fireEvent.change(screen.getByLabelText(/working directory/i), {
			target: { value: "/home/user/project" },
		});
		fireEvent.click(screen.getByRole("button", { name: /^create agent$/i }));

		await waitFor(() => {
			expect(screen.getByTestId("agent-detail")).toBeInTheDocument();
		});
		expect(posted).toMatchObject({
			name: "my-agent",
			working_dir: "/home/user/project",
			tool_policy: { mode: "allow_all" },
		});
	});

	it("hides the prompt field in interactive mode", () => {
		renderAt("/agents/new");
		expect(screen.getByLabelText(/initial prompt/i)).toBeInTheDocument();
		fireEvent.click(screen.getByRole("switch", { name: /interactive mode/i }));
		expect(screen.queryByLabelText(/initial prompt/i)).not.toBeInTheDocument();
	});
});

describe("AgentFormPage — edit", () => {
	it("prefills the form from the loaded agent and PATCHes on save", async () => {
		const agent = makeAgent({
			id: "agent-7",
			name: "edit-me",
			config: {
				working_dir: "/work",
				shell: "/bin/zsh",
				interactive: false,
				tool_policy: { mode: "allow_all" },
				env: { API_KEY: "***" },
			},
		});
		let patched: Record<string, unknown> | undefined;
		server.use(
			http.get(`${BASE}/agents/agent-7`, () => HttpResponse.json(agent)),
			http.patch(`${BASE}/agents/agent-7`, async ({ request }) => {
				patched = (await request.json()) as Record<string, unknown>;
				return HttpResponse.json({
					...agent,
					requires_restart: false,
					restarted: false,
				});
			}),
		);

		renderAt("/agents/agent-7/edit");

		const nameInput = await screen.findByLabelText(/^name$/i);
		expect(nameInput).toHaveValue("edit-me");

		fireEvent.change(nameInput, { target: { value: "renamed" } });
		fireEvent.click(screen.getByRole("button", { name: /save changes/i }));

		await waitFor(() => {
			expect(screen.getByTestId("agent-detail")).toBeInTheDocument();
		});
		expect(patched).toMatchObject({
			name: "renamed",
			working_dir: "/work",
			// Redacted env values pass through for the server-side sentinel merge.
			env: { API_KEY: "***" },
		});
	});

	it("shows the restart banner when the update requires a restart", async () => {
		const agent = makeAgent({ id: "agent-8", name: "restartable" });
		server.use(
			http.get(`${BASE}/agents/agent-8`, () => HttpResponse.json(agent)),
			http.patch(`${BASE}/agents/agent-8`, () =>
				HttpResponse.json({
					...agent,
					requires_restart: true,
					restarted: false,
				}),
			),
		);

		renderAt("/agents/agent-8/edit");
		await screen.findByLabelText(/^name$/i);
		fireEvent.click(screen.getByRole("button", { name: /save changes/i }));

		expect(
			await screen.findByText(/restart the agent to apply/i),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /restart now/i }),
		).toBeInTheDocument();
	});
});
