/**
 * AdminTable — the shared presentational table behind every product-admin view.
 *
 * Verifies the loading / error / empty / data states, the refresh control, and
 * the offset-based pagination footer (label + prev/next enable rules).
 */

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { type AdminColumn, AdminTable } from "@/components/admin/AdminTable";

interface Row {
	id: string;
	name: string;
}

const COLUMNS: AdminColumn<Row>[] = [
	{ header: "Name", render: (r) => r.name },
	{ header: "ID", render: (r) => r.id },
];

const ROWS: Row[] = [
	{ id: "1", name: "Alpha" },
	{ id: "2", name: "Beta" },
];

function renderTable(
	overrides: Partial<React.ComponentProps<typeof AdminTable<Row>>> = {},
) {
	const props = {
		title: "Widgets",
		columns: COLUMNS,
		rows: ROWS,
		rowKey: (r: Row) => r.id,
		loading: false,
		total: ROWS.length,
		offset: 0,
		limit: 50,
		onPage: vi.fn(),
		onRefresh: vi.fn(),
		...overrides,
	};
	return { props, ...render(<AdminTable<Row> {...props} />) };
}

describe("AdminTable", () => {
	it("renders the title, optional description, headers, and rows", () => {
		renderTable({ description: "All the widgets" });

		expect(
			screen.getByRole("heading", { name: "Widgets" }),
		).toBeInTheDocument();
		expect(screen.getByText("All the widgets")).toBeInTheDocument();
		expect(screen.getByText("Name")).toBeInTheDocument();
		expect(screen.getByText("Alpha")).toBeInTheDocument();
		expect(screen.getByText("Beta")).toBeInTheDocument();
	});

	it("shows a skeleton instead of rows while loading", () => {
		renderTable({ loading: true });

		// Data rows are replaced by the loading skeleton.
		expect(screen.queryByText("Alpha")).not.toBeInTheDocument();
		expect(screen.queryByText("No records found.")).not.toBeInTheDocument();
	});

	it("renders an error alert and suppresses the empty-state message", () => {
		renderTable({ rows: [], total: 0, error: "Boom" });

		const alert = screen.getByRole("alert");
		expect(alert).toHaveTextContent("Boom");
		expect(screen.queryByText("No records found.")).not.toBeInTheDocument();
	});

	it("shows the empty state when there are no rows and no error", () => {
		renderTable({ rows: [], total: 0 });

		expect(screen.getByText("No records found.")).toBeInTheDocument();
		expect(screen.getByText("0 records")).toBeInTheDocument();
	});

	it("invokes onRefresh when the refresh button is clicked", async () => {
		const onRefresh = vi.fn();
		renderTable({ onRefresh });

		await userEvent.click(
			screen.getByRole("button", { name: /refresh widgets/i }),
		);
		expect(onRefresh).toHaveBeenCalledTimes(1);
	});

	it("disables the refresh button while loading", () => {
		renderTable({ loading: true });
		expect(
			screen.getByRole("button", { name: /refresh widgets/i }),
		).toBeDisabled();
	});

	it("shows the current range and disables Previous on the first page", () => {
		renderTable({ total: 120, offset: 0, limit: 50, rows: ROWS });

		expect(screen.getByText("Showing 1–50 of 120")).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Previous" })).toBeDisabled();
		expect(screen.getByRole("button", { name: "Next" })).toBeEnabled();
	});

	it("advances the offset by one page when Next is clicked", async () => {
		const onPage = vi.fn();
		renderTable({ total: 120, offset: 0, limit: 50, onPage });

		await userEvent.click(screen.getByRole("button", { name: "Next" }));
		expect(onPage).toHaveBeenCalledWith(50);
	});

	it("clamps the offset to zero when Previous is clicked near the start", async () => {
		const onPage = vi.fn();
		renderTable({ total: 120, offset: 30, limit: 50, onPage });

		await userEvent.click(screen.getByRole("button", { name: "Previous" }));
		expect(onPage).toHaveBeenCalledWith(0);
	});

	it("disables Next on the last page and reports a partial final range", () => {
		renderTable({ total: 120, offset: 100, limit: 50, rows: ROWS });

		expect(screen.getByText("Showing 101–120 of 120")).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Next" })).toBeDisabled();
		expect(screen.getByRole("button", { name: "Previous" })).toBeEnabled();
	});
});
