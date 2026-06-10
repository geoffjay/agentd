/**
 * usePolling -- unit tests (fake timers + visibility pause/resume).
 */

import { renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { usePolling } from "@/hooks/usePolling";

/** Override document.hidden and fire a visibilitychange event */
function setDocumentHidden(hidden: boolean) {
	Object.defineProperty(document, "hidden", {
		configurable: true,
		get: () => hidden,
	});
	document.dispatchEvent(new Event("visibilitychange"));
}

describe("usePolling", () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		setDocumentHidden(false);
		vi.useRealTimers();
	});

	it("fires the callback immediately on mount", () => {
		const callback = vi.fn();
		renderHook(() => usePolling(callback, 30_000));
		expect(callback).toHaveBeenCalledTimes(1);
	});

	it("fires on each interval tick", () => {
		const callback = vi.fn();
		renderHook(() => usePolling(callback, 30_000));

		vi.advanceTimersByTime(30_000);
		expect(callback).toHaveBeenCalledTimes(2);

		vi.advanceTimersByTime(60_000);
		expect(callback).toHaveBeenCalledTimes(4);
	});

	it("respects a custom interval", () => {
		const callback = vi.fn();
		renderHook(() => usePolling(callback, 5_000));

		vi.advanceTimersByTime(4_999);
		expect(callback).toHaveBeenCalledTimes(1);
		vi.advanceTimersByTime(1);
		expect(callback).toHaveBeenCalledTimes(2);
	});

	it("pauses while the document is hidden", () => {
		const callback = vi.fn();
		renderHook(() => usePolling(callback, 30_000));
		expect(callback).toHaveBeenCalledTimes(1);

		setDocumentHidden(true);
		vi.advanceTimersByTime(120_000);
		expect(callback).toHaveBeenCalledTimes(1);
	});

	it("resumes and fires immediately when the document becomes visible", () => {
		const callback = vi.fn();
		renderHook(() => usePolling(callback, 30_000));

		setDocumentHidden(true);
		vi.advanceTimersByTime(120_000);
		expect(callback).toHaveBeenCalledTimes(1);

		setDocumentHidden(false);
		// Immediate fire on visibility
		expect(callback).toHaveBeenCalledTimes(2);
		// ...and the interval resumes
		vi.advanceTimersByTime(30_000);
		expect(callback).toHaveBeenCalledTimes(3);
	});

	it("stops polling after unmount", () => {
		const callback = vi.fn();
		const { unmount } = renderHook(() => usePolling(callback, 30_000));

		unmount();
		vi.advanceTimersByTime(120_000);
		expect(callback).toHaveBeenCalledTimes(1);
	});

	it("always invokes the latest callback", () => {
		const first = vi.fn();
		const second = vi.fn();
		const { rerender } = renderHook(({ cb }) => usePolling(cb, 30_000), {
			initialProps: { cb: first },
		});

		rerender({ cb: second });
		vi.advanceTimersByTime(30_000);

		expect(first).toHaveBeenCalledTimes(1); // mount fire only
		expect(second).toHaveBeenCalledTimes(1); // interval fire
	});
});
