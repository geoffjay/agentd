/**
 * CreateAgentDialog tests.
 *
 * Covers rendering, interactive-mode toggle behaviour, the Advanced
 * collapsible section, form submission, and validation.
 */

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CreateAgentDialog } from "@/pages/agents/CreateAgentDialog";
import type { CreateAgentRequest } from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderDialog(
	props: Partial<Parameters<typeof CreateAgentDialog>[0]> = {},
) {
	const onClose = vi.fn();
	const onCreate = vi.fn().mockResolvedValue(undefined);

	render(
		<CreateAgentDialog
			open={true}
			onClose={onClose}
			onCreate={onCreate}
			{...props}
		/>,
	);

	return { onClose, onCreate };
}

/** Open the Advanced collapsible section. */
function openAdvanced() {
	fireEvent.click(screen.getByRole("button", { name: /advanced/i }));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("CreateAgentDialog", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	// ── Rendering ────────────────────────────────────────────────────────────

	it("renders the dialog with the correct title", () => {
		renderDialog();
		expect(
			screen.getByRole("dialog", { name: /create agent/i }),
		).toBeInTheDocument();
	});

	it("does not render when open=false", () => {
		renderDialog({ open: false });
		expect(screen.queryByRole("dialog")).toBeNull();
	});

	it("renders required fields: name and working directory", () => {
		renderDialog();
		expect(screen.getByLabelText(/name/i)).toBeInTheDocument();
		expect(screen.getByLabelText(/working directory/i)).toBeInTheDocument();
	});

	// ── Advanced section ─────────────────────────────────────────────────────

	it("Advanced section is collapsed by default", () => {
		renderDialog();
		expect(screen.queryByLabelText(/interactive mode/i)).toBeNull();
	});

	it("Advanced section expands when the header is clicked", () => {
		renderDialog();
		openAdvanced();
		expect(
			screen.getByRole("switch", { name: /interactive mode/i }),
		).toBeInTheDocument();
	});

	it("Advanced section collapses when the header is clicked again", () => {
		renderDialog();
		openAdvanced();
		fireEvent.click(screen.getByRole("button", { name: /advanced/i }));
		expect(
			screen.queryByRole("switch", { name: /interactive mode/i }),
		).toBeNull();
	});

	it("shell and environment variable fields are inside the Advanced section", () => {
		renderDialog();
		// Fields not visible before opening
		expect(screen.queryByLabelText(/shell/i)).toBeNull();
		openAdvanced();
		expect(screen.getByLabelText(/shell/i)).toBeInTheDocument();
		expect(
			screen.getByLabelText(/environment variable key 1/i),
		).toBeInTheDocument();
	});

	// ── Interactive mode toggle ───────────────────────────────────────────────

	it("interactive toggle defaults to off (aria-checked=false)", () => {
		renderDialog();
		openAdvanced();
		expect(
			screen.getByRole("switch", { name: /interactive mode/i }),
		).toHaveAttribute("aria-checked", "false");
	});

	it("toggling interactive on changes aria-checked to true", () => {
		renderDialog();
		openAdvanced();
		fireEvent.click(screen.getByRole("switch", { name: /interactive mode/i }));
		expect(
			screen.getByRole("switch", { name: /interactive mode/i }),
		).toHaveAttribute("aria-checked", "true");
	});

	it("shows the interactive mode description text", () => {
		renderDialog();
		openAdvanced();
		expect(
			screen.getByText(/runs claude without the sdk protocol/i),
		).toBeInTheDocument();
	});

	it("shows a warning note when interactive mode is enabled", () => {
		renderDialog();
		openAdvanced();
		// No warning before toggling
		expect(screen.queryByRole("note")).toBeNull();
		fireEvent.click(screen.getByRole("switch", { name: /interactive mode/i }));
		expect(screen.getByRole("note")).toBeInTheDocument();
		expect(screen.getByRole("note")).toHaveTextContent(
			/cost tracking and tool policies/i,
		);
	});

	it("hides the warning note when interactive mode is toggled back off", () => {
		renderDialog();
		openAdvanced();
		fireEvent.click(screen.getByRole("switch", { name: /interactive mode/i }));
		fireEvent.click(screen.getByRole("switch", { name: /interactive mode/i }));
		expect(screen.queryByRole("note")).toBeNull();
	});

	it("hides the initial Prompt field when interactive mode is enabled", () => {
		renderDialog();
		// Use placeholder text to unambiguously identify the initial prompt textarea
		// (avoids matching "System Prompt" label which also contains "prompt").
		expect(
			screen.getByPlaceholderText(/initial prompt for the agent/i),
		).toBeInTheDocument();
		openAdvanced();
		fireEvent.click(screen.getByRole("switch", { name: /interactive mode/i }));
		expect(
			screen.queryByPlaceholderText(/initial prompt for the agent/i),
		).toBeNull();
	});

	// ── Form submission ───────────────────────────────────────────────────────

	it("calls onCreate with interactive=false by default", async () => {
		const { onCreate } = renderDialog();

		fireEvent.change(screen.getByLabelText(/name/i), {
			target: { value: "my-agent" },
		});
		fireEvent.change(screen.getByLabelText(/working directory/i), {
			target: { value: "/tmp/work" },
		});
		fireEvent.click(screen.getByRole("button", { name: /create agent/i }));

		await waitFor(() => expect(onCreate).toHaveBeenCalledOnce());
		const request = onCreate.mock.calls[0][0] as CreateAgentRequest;
		expect(request.interactive).toBe(false);
	});

	it("calls onCreate with interactive=true when toggled on", async () => {
		const { onCreate } = renderDialog();

		fireEvent.change(screen.getByLabelText(/name/i), {
			target: { value: "pty-agent" },
		});
		fireEvent.change(screen.getByLabelText(/working directory/i), {
			target: { value: "/tmp/work" },
		});
		openAdvanced();
		fireEvent.click(screen.getByRole("switch", { name: /interactive mode/i }));
		fireEvent.click(screen.getByRole("button", { name: /create agent/i }));

		await waitFor(() => expect(onCreate).toHaveBeenCalledOnce());
		const request = onCreate.mock.calls[0][0] as CreateAgentRequest;
		expect(request.interactive).toBe(true);
	});

	// ── Validation ────────────────────────────────────────────────────────────

	it("shows validation errors when required fields are empty", async () => {
		renderDialog();
		fireEvent.click(screen.getByRole("button", { name: /create agent/i }));
		await waitFor(() =>
			expect(screen.getByText(/name is required/i)).toBeInTheDocument(),
		);
		expect(
			screen.getByText(/working directory is required/i),
		).toBeInTheDocument();
	});

	it("calls onClose when Cancel is clicked", () => {
		const { onClose } = renderDialog();
		fireEvent.click(screen.getByRole("button", { name: /cancel/i }));
		expect(onClose).toHaveBeenCalledOnce();
	});
});
