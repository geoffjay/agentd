/**
 * Tests for SearchResultsTable and CodePreviewDrawer components.
 */

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SearchResultsTable } from "@/components/index/SearchResultsTable";
import {
	makeSearchResultItem,
	makeSearchResultList,
} from "@/test/mocks/factories/codeindex";

// ShikiHighlighter makes async calls that aren't needed in unit tests
vi.mock("react-shiki", () => ({
	default: ({ children }: { children: string }) => (
		<pre data-testid="highlighted-code">{children}</pre>
	),
}));

describe("SearchResultsTable", () => {
	it("renders empty state when no results", () => {
		render(<SearchResultsTable results={[]} loading={false} />);
		expect(screen.getByText("No results")).toBeTruthy();
	});

	it("renders a row for each result", () => {
		const results = makeSearchResultList(3);
		render(<SearchResultsTable results={results} loading={false} />);
		// Each result has a unique symbol_name like function_N
		expect(screen.getByText("function_1")).toBeTruthy();
		expect(screen.getByText("function_2")).toBeTruthy();
		expect(screen.getByText("function_3")).toBeTruthy();
	});

	it("renders score pills", () => {
		const result = makeSearchResultItem({ score: 0.9 });
		render(<SearchResultsTable results={[result]} loading={false} />);
		expect(screen.getByText("90%")).toBeTruthy();
	});

	it("renders language badge", () => {
		const result = makeSearchResultItem({ language: "typescript" });
		render(<SearchResultsTable results={[result]} loading={false} />);
		expect(screen.getByText("typescript")).toBeTruthy();
	});

	it("renders dash for results without symbol_name", () => {
		const result = makeSearchResultItem({ symbol_name: undefined });
		render(<SearchResultsTable results={[result]} loading={false} />);
		expect(screen.getByText("—")).toBeTruthy();
	});

	it("opens drawer with metadata on row click", async () => {
		const result = makeSearchResultItem({
			symbol_name: "my_function",
			file_path: "src/lib.rs",
			language: "rust",
			start_line: 5,
			end_line: 15,
			score: 0.75,
			repo_id: "repo-abc",
		});
		render(<SearchResultsTable results={[result]} loading={false} />);

		fireEvent.click(screen.getByText("my_function"));

		await waitFor(() => {
			expect(screen.getByRole("dialog")).toBeTruthy();
		});

		// symbol name appears in both the table row and the drawer — use getAllByText
		expect(screen.getAllByText("my_function").length).toBeGreaterThanOrEqual(1);
		// Metadata rows visible in drawer
		expect(screen.getByText("repo-abc")).toBeTruthy();
		expect(screen.getByText("5 – 15")).toBeTruthy();
	});

	it("drawer shows summary when present", async () => {
		const result = makeSearchResultItem({
			summary: "This function handles authentication.",
		});
		render(<SearchResultsTable results={[result]} loading={false} />);

		fireEvent.click(screen.getByText(result.symbol_name!));

		await waitFor(() => expect(screen.getByRole("dialog")).toBeTruthy());
		expect(
			screen.getByText("This function handles authentication."),
		).toBeTruthy();
	});

	it("drawer shows highlighted code", async () => {
		const result = makeSearchResultItem({
			content: "fn answer() -> u32 { 42 }",
		});
		render(<SearchResultsTable results={[result]} loading={false} />);

		fireEvent.click(screen.getByText(result.symbol_name!));

		await waitFor(() => expect(screen.getByRole("dialog")).toBeTruthy());
		expect(screen.getByTestId("highlighted-code")).toBeTruthy();
		expect(screen.getByText("fn answer() -> u32 { 42 }")).toBeTruthy();
	});

	it("closes drawer on backdrop click", async () => {
		const result = makeSearchResultItem();
		render(<SearchResultsTable results={[result]} loading={false} />);

		fireEvent.click(screen.getByText(result.symbol_name!));
		await waitFor(() => expect(screen.getByRole("dialog")).toBeTruthy());

		// Click the backdrop (aria-hidden overlay)
		const backdrop = document.querySelector(
			'[aria-hidden="true"]',
		) as HTMLElement;
		fireEvent.click(backdrop);

		await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
	});

	it("uses file name as drawer title when symbol_name is absent", async () => {
		const result = makeSearchResultItem({
			symbol_name: undefined,
			file_path: "src/utils/helpers.ts",
		});
		render(<SearchResultsTable results={[result]} loading={false} />);

		// Click the file name cell (the "helpers.ts" part)
		const row = screen.getByText("helpers.ts").closest("tr")!;
		fireEvent.click(row);

		await waitFor(() => expect(screen.getByRole("dialog")).toBeTruthy());
		// Drawer label should contain the filename
		expect(screen.getByLabelText("helpers.ts")).toBeTruthy();
	});
});
