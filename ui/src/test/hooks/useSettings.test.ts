/**
 * useSettings -- unit tests.
 */

import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { mockLoadSettings, mockSaveSettings, mockResetSettings } = vi.hoisted(() => ({
	mockLoadSettings: vi.fn(),
	mockSaveSettings: vi.fn(),
	mockResetSettings: vi.fn(),
}));

vi.mock("@/stores/settingsStore", () => ({
	loadSettings: mockLoadSettings,
	saveSettings: mockSaveSettings,
	resetSettings: mockResetSettings,
}));

import { useSettings } from "@/hooks/useSettings";

const DEFAULT_SETTINGS = {
	services: {
		orchestratorUrl: "http://localhost:17006",
		notifyUrl: "http://localhost:17004",
		askUrl: "http://localhost:17001",
		memoryUrl: "http://localhost:17008",
	},
	ui: { theme: "agentd-dark", sidebarDefaultOpen: false },
};

describe("useSettings", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		mockLoadSettings.mockReturnValue(DEFAULT_SETTINGS);
		mockResetSettings.mockReturnValue(DEFAULT_SETTINGS);
	});

	it("loads settings on mount", () => {
		const { result } = renderHook(() => useSettings());
		expect(result.current.settings).toEqual(DEFAULT_SETTINGS);
		expect(mockLoadSettings).toHaveBeenCalled();
	});

	it("update merges top-level settings and persists", () => {
		const { result } = renderHook(() => useSettings());
		act(() => {
			result.current.update({ version: 2 });
		});
		expect(result.current.settings.version).toBe(2);
		expect(mockSaveSettings).toHaveBeenCalled();
	});

	it("updateServices merges only services and persists", () => {
		const { result } = renderHook(() => useSettings());
		act(() => {
			result.current.updateServices({ orchestratorUrl: "http://custom:9999" });
		});
		expect(result.current.settings.services.orchestratorUrl).toBe("http://custom:9999");
		expect(result.current.settings.services.notifyUrl).toBe("http://localhost:17004");
		expect(mockSaveSettings).toHaveBeenCalled();
	});

	it("updateUI merges only ui and persists", () => {
		const { result } = renderHook(() => useSettings());
		act(() => {
			result.current.updateUI({ theme: "tokyo-night" });
		});
		expect(result.current.settings.ui.theme).toBe("tokyo-night");
		expect(result.current.settings.ui.sidebarDefaultOpen).toBe(false);
		expect(mockSaveSettings).toHaveBeenCalled();
	});

	it("reset restores defaults", () => {
		const { result } = renderHook(() => useSettings());
		act(() => {
			result.current.updateUI({ theme: "custom" });
		});
		act(() => {
			result.current.reset();
		});
		expect(mockResetSettings).toHaveBeenCalled();
		expect(result.current.settings).toEqual(DEFAULT_SETTINGS);
	});
});
