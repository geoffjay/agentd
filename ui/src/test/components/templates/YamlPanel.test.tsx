/**
 * YamlPanel — export preview, copy, import, and error handling.
 */

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { YamlPanel } from "@/components/templates/YamlPanel";

function renderPanel(overrides: Partial<Parameters<typeof YamlPanel>[0]> = {}) {
	const onImport = vi.fn().mockReturnValue([]);
	render(
		<YamlPanel
			title="Agent template"
			exportedYaml={"name: preview\nworking_dir: ."}
			exportWarnings={[]}
			onImport={onImport}
			{...overrides}
		/>,
	);
	return { onImport };
}

describe("YamlPanel", () => {
	it("renders the export preview", async () => {
		renderPanel();
		expect(screen.getByText("Agent template")).toBeInTheDocument();
		// Shiki splits highlighted code across spans; assert on text content.
		await waitFor(() => {
			expect(document.body.textContent).toContain("name: preview");
		});
	});

	it("copies the export to the clipboard", async () => {
		const writeText = vi.fn().mockResolvedValue(undefined);
		Object.assign(navigator, { clipboard: { writeText } });

		renderPanel();
		fireEvent.click(screen.getByRole("button", { name: /copy yaml/i }));

		await waitFor(() => {
			expect(writeText).toHaveBeenCalledWith("name: preview\nworking_dir: .");
		});
	});

	it("imports pasted YAML and shows returned warnings", () => {
		const { onImport } = renderPanel();
		onImport.mockReturnValue(["Room roles were dropped."]);

		fireEvent.change(screen.getByLabelText(/yaml template to import/i), {
			target: { value: "name: pasted" },
		});
		fireEvent.click(screen.getByRole("button", { name: /import into form/i }));

		expect(onImport).toHaveBeenCalledWith("name: pasted");
		expect(screen.getByText(/room roles were dropped/i)).toBeInTheDocument();
	});

	it("shows parse errors inline", () => {
		const { onImport } = renderPanel();
		onImport.mockImplementation(() => {
			throw new Error("Template must be a YAML mapping.");
		});

		fireEvent.change(screen.getByLabelText(/yaml template to import/i), {
			target: { value: "- not a mapping" },
		});
		fireEvent.click(screen.getByRole("button", { name: /import into form/i }));

		expect(screen.getByText(/must be a yaml mapping/i)).toBeInTheDocument();
	});

	it("renders export warnings", () => {
		renderPanel({ exportWarnings: ["Env values are redacted."] });
		expect(screen.getByText(/env values are redacted/i)).toBeInTheDocument();
	});
});
