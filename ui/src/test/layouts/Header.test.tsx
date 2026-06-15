import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ThemeProvider } from "@/hooks/useTheme";
import type { LayoutContextValue } from "@/layouts/context";
import { LayoutContext } from "@/layouts/context";
import { Header } from "@/layouts/Header";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeContext(
	overrides: Partial<LayoutContextValue> = {},
): LayoutContextValue {
	return {
		sidebarOpen: true,
		setSidebarOpen: vi.fn(),
		toggleSidebar: vi.fn(),
		searchOpen: false,
		openSearch: vi.fn(),
		closeSearch: vi.fn(),
		...overrides,
	};
}

function renderHeader(
	props = {},
	contextOverrides: Partial<LayoutContextValue> = {},
) {
	const ctx = makeContext(contextOverrides);
	return {
		ctx,
		...render(
			<ThemeProvider>
				<MemoryRouter>
					<LayoutContext.Provider value={ctx}>
						<Header {...props} />
					</LayoutContext.Provider>
				</MemoryRouter>
			</ThemeProvider>,
		),
	};
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("Header", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it("renders a header element", () => {
		renderHeader();
		expect(screen.getByRole("banner")).toBeInTheDocument();
	});

	it("renders the sidebar toggle button", () => {
		renderHeader();
		expect(
			screen.getByRole("button", { name: /toggle sidebar/i }),
		).toBeInTheDocument();
	});

	it("calls toggleSidebar when hamburger button is clicked", () => {
		const { ctx } = renderHeader();
		const btn = screen.getByRole("button", { name: /toggle sidebar/i });
		fireEvent.click(btn);
		expect(ctx.toggleSidebar).toHaveBeenCalledOnce();
	});

	it("renders notification link", () => {
		renderHeader();
		expect(
			screen.getByRole("link", { name: /notifications/i }),
		).toBeInTheDocument();
	});

	it("renders the user menu button", () => {
		renderHeader();
		expect(
			screen.getByRole("button", { name: /user menu/i }),
		).toBeInTheDocument();
	});

	it("reveals settings and logout when the user menu is opened", () => {
		renderHeader();
		// Settings/logout live inside the dropdown, hidden until opened.
		expect(
			screen.queryByRole("link", { name: /settings/i }),
		).not.toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: /user menu/i }));
		expect(screen.getByRole("link", { name: /settings/i })).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /log out/i }),
		).toBeInTheDocument();
	});

	it("shows notification badge when unreadCount > 0", () => {
		renderHeader({ unreadCount: 5 });
		expect(screen.getByLabelText("5 unread notifications")).toBeInTheDocument();
	});

	it("does not show notification badge when unreadCount is 0", () => {
		renderHeader({ unreadCount: 0 });
		expect(
			screen.queryByLabelText(/unread notifications/i),
		).not.toBeInTheDocument();
	});

	it("shows 99+ for large unread counts", () => {
		renderHeader({ unreadCount: 150 });
		expect(screen.getByText("99+")).toBeInTheDocument();
	});

	it("renders the global search button", () => {
		renderHeader();
		expect(
			screen.getByRole("button", { name: /global search/i }),
		).toBeInTheDocument();
	});

	it("calls openSearch when the search button is clicked", () => {
		const { ctx } = renderHeader();
		fireEvent.click(screen.getByRole("button", { name: /global search/i }));
		expect(ctx.openSearch).toHaveBeenCalledOnce();
	});

	it("search button has Ctrl+K keyboard shortcut hint", () => {
		renderHeader();
		const btn = screen.getByRole("button", { name: /global search/i });
		expect(btn).toHaveAttribute("aria-keyshortcuts", "Control+k Meta+k");
	});

	it("renders the theme toggle button", () => {
		renderHeader();
		expect(screen.getByRole("button", { name: /theme/i })).toBeInTheDocument();
	});
});
