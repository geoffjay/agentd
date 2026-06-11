/**
 * WorkflowFormPage — create and edit flows against the MSW handlers,
 * including trigger type switching and trigger-specific variables.
 */

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { HttpResponse, http } from "msw";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { WorkflowFormPage } from "@/pages/workflows/WorkflowFormPage";
import { makeTrigger, makeWorkflow } from "@/test/mocks/factories";
import { server } from "@/test/mocks/server";

const BASE = "http://localhost:17006";

function renderAt(path: string) {
	return render(
		<MemoryRouter initialEntries={[path]}>
			<Routes>
				<Route path="/workflows/new" element={<WorkflowFormPage />} />
				<Route path="/workflows/:id/edit" element={<WorkflowFormPage />} />
				<Route
					path="/workflows/:id"
					element={<div data-testid="workflow-detail" />}
				/>
				<Route
					path="/workflows"
					element={<div data-testid="workflow-list" />}
				/>
			</Routes>
		</MemoryRouter>,
	);
}

function selectTrigger(value: string) {
	fireEvent.change(screen.getByLabelText(/trigger type/i), {
		target: { value },
	});
}

describe("WorkflowFormPage — create", () => {
	it("defaults to GitHub Issues with poll interval visible", () => {
		renderAt("/workflows/new");
		expect(screen.getByText("Create Workflow")).toBeInTheDocument();
		expect(screen.getByLabelText(/trigger type/i)).toHaveValue("github_issues");
		expect(screen.getByLabelText(/owner/i)).toBeInTheDocument();
		expect(screen.getByLabelText(/poll interval/i)).toBeInTheDocument();
	});

	it("switches trigger fields per type and hides poll interval for non-polling triggers", () => {
		renderAt("/workflows/new");

		selectTrigger("cron");
		expect(screen.getByLabelText(/cron expression/i)).toBeInTheDocument();
		expect(screen.queryByLabelText(/poll interval/i)).not.toBeInTheDocument();

		selectTrigger("manual");
		expect(
			screen.getAllByText(/dispatched explicitly/i).length,
		).toBeGreaterThan(0);
		expect(screen.queryByLabelText(/poll interval/i)).not.toBeInTheDocument();
	});

	it("restores cached values when switching back to a previous trigger type", () => {
		renderAt("/workflows/new");
		fireEvent.change(screen.getByLabelText(/owner/i), {
			target: { value: "geoffjay" },
		});

		selectTrigger("cron");
		selectTrigger("github_issues");

		expect(screen.getByLabelText(/owner/i)).toHaveValue("geoffjay");
	});

	it("renders the composite editor with two starter sub-triggers", () => {
		renderAt("/workflows/new");
		selectTrigger("composite");

		expect(screen.getByText("Sub-trigger 1")).toBeInTheDocument();
		expect(screen.getByText("Sub-trigger 2")).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /add sub-trigger/i }),
		).toBeInTheDocument();
	});

	it("shows trigger-specific prompt variables", () => {
		renderAt("/workflows/new");
		selectTrigger("cron");

		fireEvent.click(
			screen.getByRole("button", { name: /available variables/i }),
		);
		expect(screen.getAllByText("{{cron_expression}}").length).toBeGreaterThan(
			0,
		);
	});

	it("POSTs the request and navigates to the created workflow", async () => {
		let posted: Record<string, unknown> | undefined;
		server.use(
			http.post(`${BASE}/workflows`, async ({ request }) => {
				posted = (await request.json()) as Record<string, unknown>;
				return HttpResponse.json(
					makeWorkflow({ id: "wf-created", name: String(posted.name) }),
					{ status: 201 },
				);
			}),
		);

		renderAt("/workflows/new");
		fireEvent.change(screen.getByLabelText(/workflow name/i), {
			target: { value: "issue-bot" },
		});
		// Default MSW agents are all running; pick the first one.
		const agentSelect = screen.getByLabelText(/^agent$/i) as HTMLSelectElement;
		await waitFor(() => {
			expect(agentSelect.options.length).toBeGreaterThan(1);
		});
		fireEvent.change(agentSelect, {
			target: { value: agentSelect.options[1].value },
		});
		fireEvent.change(screen.getByLabelText(/owner/i), {
			target: { value: "geoffjay" },
		});
		fireEvent.change(screen.getByLabelText(/repository/i), {
			target: { value: "agentd" },
		});

		fireEvent.click(screen.getByRole("button", { name: /create workflow/i }));

		await waitFor(() => {
			expect(screen.getByTestId("workflow-detail")).toBeInTheDocument();
		});
		expect(posted).toMatchObject({
			name: "issue-bot",
			trigger_config: {
				type: "github_issues",
				owner: "geoffjay",
				repo: "agentd",
			},
		});
	});
});

describe("WorkflowFormPage — edit", () => {
	it("prefills from the loaded workflow and PUTs trigger_config on save", async () => {
		const workflow = makeWorkflow({
			id: "wf-9",
			name: "nightly",
			trigger_config: makeTrigger("cron"),
		});
		let updated: Record<string, unknown> | undefined;
		server.use(
			http.get(`${BASE}/workflows/wf-9`, () => HttpResponse.json(workflow)),
			http.put(`${BASE}/workflows/wf-9`, async ({ request }) => {
				updated = (await request.json()) as Record<string, unknown>;
				return HttpResponse.json(workflow);
			}),
		);

		renderAt("/workflows/wf-9/edit");

		const nameInput = await screen.findByLabelText(/workflow name/i);
		expect(nameInput).toHaveValue("nightly");
		expect(screen.getByLabelText(/trigger type/i)).toHaveValue("cron");
		expect(screen.getByLabelText(/cron expression/i)).toHaveValue(
			"0 9 * * MON-FRI",
		);

		fireEvent.change(screen.getByLabelText(/cron expression/i), {
			target: { value: "0 12 * * *" },
		});
		fireEvent.click(screen.getByRole("button", { name: /save changes/i }));

		await waitFor(() => {
			expect(screen.getByTestId("workflow-detail")).toBeInTheDocument();
		});
		expect(updated).toMatchObject({
			name: "nightly",
			trigger_config: { type: "cron", expression: "0 12 * * *" },
			agent_id: workflow.agent_id,
		});
	});
});
