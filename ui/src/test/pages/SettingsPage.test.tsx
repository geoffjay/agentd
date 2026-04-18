/**
 * SettingsPage — smoke tests for rendering and data management actions.
 */

import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { mockReset, mockResetSettings } = vi.hoisted(() => ({
	mockReset: vi.fn(),
	mockResetSettings: vi.fn(),
}));

vi.mock("@/hooks/useSettings", () => ({
	useSettings: () => ({
		settings: {
			services: {
				orchestratorUrl: "http://localhost:17006",
				notifyUrl: "http://localhost:17004",
				askUrl: "http://localhost:17001",
				memoryUrl: "http://localhost:17008",
			},
			ui: { theme: "dark", pageSize: 20 },
		},
		updateServices: vi.fn(),
		updateUI: vi.fn(),
		reset: mockReset,
	}),
}));

vi.mock("@/stores/settingsStore", async (importOriginal) => {
	const actual = await importOriginal<Record<string, unknown>>();
	return { ...actual, resetSettings: mockResetSettings };
});

vi.mock("@/hooks/useTheme", () => ({
	useTheme: () => ({
		themeId: "agentd-dark",
		resolvedThemeId: "agentd-dark",
		theme: {},
		setTheme: vi.fn(),
	}),
	ThemeProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

import { SettingsPage } from "@/pages/SettingsPage";

function renderPage() {
	return render(
		<MemoryRouter>
			<SettingsPage />
		</MemoryRouter>,
	);
}

describe("SettingsPage", () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it("renders all section headings", () => {
		renderPage();
		expect(screen.getByText("Settings")).toBeInTheDocument();
		expect(screen.getByText("Service Configuration")).toBeInTheDocument();
		expect(screen.getByText("UI Preferences")).toBeInTheDocument();
		expect(screen.getByText("About")).toBeInTheDocument();
		expect(screen.getByText("Data Management")).toBeInTheDocument();
	});

	it("renders Clear, Export, and Import buttons", () => {
		renderPage();
		expect(screen.getByText("Clear All Settings")).toBeInTheDocument();
		expect(screen.getByText("Export Settings")).toBeInTheDocument();
		expect(screen.getByText("Import Settings")).toBeInTheDocument();
	});

	it("requires two clicks to clear settings (confirmation)", () => {
		renderPage();
		const btn = screen.getByText("Clear All Settings");
		fireEvent.click(btn);
		expect(screen.getByText("Confirm Clear All Settings")).toBeInTheDocument();
		expect(mockResetSettings).not.toHaveBeenCalled();

		fireEvent.click(screen.getByText("Confirm Clear All Settings"));
		expect(mockResetSettings).toHaveBeenCalled();
		expect(mockReset).toHaveBeenCalled();
	});

	it("renders hidden file input for import", () => {
		renderPage();
		const input = screen.getByLabelText("Import settings file");
		expect(input).toBeInTheDocument();
		expect(input).toHaveAttribute("type", "file");
	});
});
