/**
 * useFocusTrap -- unit tests.
 */

import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useFocusTrap } from "@/hooks/useFocusTrap";

describe("useFocusTrap", () => {
	it("returns a ref object", () => {
		const { result } = renderHook(() => useFocusTrap({ active: false }));
		expect(result.current).toBeDefined();
		expect(result.current.current).toBeNull();
	});

	it("attaches keydown listener when active", () => {
		const addSpy = vi.spyOn(document, "addEventListener");
		renderHook(() => useFocusTrap({ active: true }));
		expect(addSpy).toHaveBeenCalledWith("keydown", expect.any(Function));
		addSpy.mockRestore();
	});

	it("cleans up keydown listener on unmount", () => {
		const removeSpy = vi.spyOn(document, "removeEventListener");
		const { unmount } = renderHook(() => useFocusTrap({ active: true }));
		unmount();
		expect(removeSpy).toHaveBeenCalledWith("keydown", expect.any(Function));
		removeSpy.mockRestore();
	});

	it("calls onEscape when Escape is pressed", () => {
		const onEscape = vi.fn();
		const { result } = renderHook(() =>
			useFocusTrap({ active: true, onEscape }),
		);

		const container = document.createElement("div");
		const button = document.createElement("button");
		container.appendChild(button);
		document.body.appendChild(container);
		(result.current as { current: HTMLElement | null }).current = container;

		document.dispatchEvent(
			new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
		);

		expect(onEscape).toHaveBeenCalled();
		document.body.removeChild(container);
	});
});
