/**
 * Tests for RepositoryList component.
 */

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RepositoryList } from "@/components/index/RepositoryList";
import { makeRepoRecord } from "@/test/mocks/factories/codeindex";

describe("RepositoryList", () => {
	const defaultProps = {
		repositories: [],
		loading: false,
		busyIds: new Set<string>(),
		onReindex: vi.fn().mockResolvedValue(true),
		onDelete: vi.fn().mockResolvedValue(true),
	};

	it("renders empty state when no repos", () => {
		render(<RepositoryList {...defaultProps} />);
		expect(screen.getByText("No repositories indexed")).toBeTruthy();
	});

	it("renders repo name and path", () => {
		const repo = makeRepoRecord({ name: "agentd", path: "/projects/agentd" });
		render(<RepositoryList {...defaultProps} repositories={[repo]} />);
		expect(screen.getByText("agentd")).toBeTruthy();
		expect(screen.getByText("/projects/agentd")).toBeTruthy();
	});

	it("renders status badge for each repo", () => {
		const repos = [
			makeRepoRecord({ status: "ready" }),
			makeRepoRecord({ status: "indexing" }),
			makeRepoRecord({ status: "error" }),
			makeRepoRecord({ status: "pending" }),
		];
		render(<RepositoryList {...defaultProps} repositories={repos} />);
		expect(screen.getByText("Ready")).toBeTruthy();
		expect(screen.getByText("Indexing")).toBeTruthy();
		expect(screen.getByText("Error")).toBeTruthy();
		expect(screen.getByText("Pending")).toBeTruthy();
	});

	it("renders last indexed date when present", () => {
		const repo = makeRepoRecord({
			last_indexed: "2024-06-15T09:30:00Z",
		});
		render(<RepositoryList {...defaultProps} repositories={[repo]} />);
		// Date is localized; just check "Never" is not shown
		expect(screen.queryByText("Never")).toBeNull();
	});

	it("renders Never when last_indexed is absent", () => {
		const repo = makeRepoRecord({ last_indexed: undefined });
		render(<RepositoryList {...defaultProps} repositories={[repo]} />);
		expect(screen.getByText("Never")).toBeTruthy();
	});

	it("calls onReindex when reindex button clicked", async () => {
		const onReindex = vi.fn().mockResolvedValue(true);
		const repo = makeRepoRecord({ id: "repo-r1", name: "myrepo" });
		render(
			<RepositoryList
				{...defaultProps}
				repositories={[repo]}
				onReindex={onReindex}
			/>,
		);
		fireEvent.click(screen.getByLabelText("Reindex myrepo"));
		await waitFor(() => expect(onReindex).toHaveBeenCalledWith("repo-r1"));
	});

	it("opens delete confirmation dialog and calls onDelete on confirm", async () => {
		const onDelete = vi.fn().mockResolvedValue(true);
		const repo = makeRepoRecord({ id: "repo-d1", name: "deleteme" });
		render(
			<RepositoryList
				{...defaultProps}
				repositories={[repo]}
				onDelete={onDelete}
			/>,
		);

		fireEvent.click(screen.getByLabelText("Delete deleteme"));
		// Confirmation dialog should appear (ConfirmDialog uses role="alertdialog")
		expect(screen.getByRole("alertdialog")).toBeTruthy();
		// repo name appears in both the table row and the dialog description
		expect(screen.getAllByText(/deleteme/).length).toBeGreaterThanOrEqual(1);

		// Click the confirm button
		fireEvent.click(screen.getByText("Delete"));
		await waitFor(() => expect(onDelete).toHaveBeenCalledWith("repo-d1"));
	});

	it("dismisses delete dialog on cancel", async () => {
		const onDelete = vi.fn();
		const repo = makeRepoRecord({ name: "cancelme" });
		render(
			<RepositoryList
				{...defaultProps}
				repositories={[repo]}
				onDelete={onDelete}
			/>,
		);

		fireEvent.click(screen.getByLabelText("Delete cancelme"));
		expect(screen.getByRole("alertdialog")).toBeTruthy();

		fireEvent.click(screen.getByText("Cancel"));
		await waitFor(() =>
			expect(screen.queryByRole("dialog")).toBeNull(),
		);
		expect(onDelete).not.toHaveBeenCalled();
	});

	it("disables action buttons for busy repo IDs", () => {
		const repo = makeRepoRecord({ id: "repo-busy", name: "busy" });
		render(
			<RepositoryList
				{...defaultProps}
				repositories={[repo]}
				busyIds={new Set(["repo-busy"])}
			/>,
		);
		expect(
			(screen.getByLabelText("Reindex busy") as HTMLButtonElement).disabled,
		).toBe(true);
		expect(
			(screen.getByLabelText("Delete busy") as HTMLButtonElement).disabled,
		).toBe(true);
	});
});
