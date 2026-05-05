/**
 * Toast -- unit tests.
 */

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Toast } from "@/components/common/Toast";
import type { Toast as ToastData } from "@/stores/toastStore";

function makeToast(overrides: Partial<ToastData> = {}): ToastData {
	return {
		id: "t-1",
		type: "info",
		title: "Test toast",
		duration: 0,
		createdAt: Date.now(),
		...overrides,
	};
}

describe("Toast", () => {
	it("renders the title", () => {
		render(<Toast toast={makeToast()} onDismiss={vi.fn()} />);
		expect(screen.getByText("Test toast")).toBeInTheDocument();
	});

	it("renders optional message", () => {
		render(
			<Toast
				toast={makeToast({ message: "Extra detail" })}
				onDismiss={vi.fn()}
			/>,
		);
		expect(screen.getByText("Extra detail")).toBeInTheDocument();
	});

	it("calls onDismiss when X button is clicked", () => {
		const onDismiss = vi.fn();
		render(<Toast toast={makeToast()} onDismiss={onDismiss} />);
		fireEvent.click(screen.getByRole("button", { name: /dismiss/i }));
		expect(onDismiss).toHaveBeenCalledWith("t-1");
	});

	it("renders success icon for success type", () => {
		render(
			<Toast toast={makeToast({ type: "success" })} onDismiss={vi.fn()} />,
		);
		expect(screen.getByRole("alert")).toBeInTheDocument();
	});

	it("renders error icon for error type", () => {
		render(
			<Toast toast={makeToast({ type: "error" })} onDismiss={vi.fn()} />,
		);
		const alert = screen.getByRole("alert");
		expect(alert.getAttribute("aria-live")).toBe("assertive");
	});

	it("renders warning icon for warning type", () => {
		render(
			<Toast toast={makeToast({ type: "warning" })} onDismiss={vi.fn()} />,
		);
		expect(screen.getByRole("alert")).toBeInTheDocument();
	});

	it("renders action button when action is provided", () => {
		const onClick = vi.fn();
		render(
			<Toast
				toast={makeToast({ action: { label: "Retry", onClick } })}
				onDismiss={vi.fn()}
			/>,
		);
		expect(screen.getByText("Retry")).toBeInTheDocument();
	});

	it("calls action onClick and dismisses when action button is clicked", () => {
		const onClick = vi.fn();
		const onDismiss = vi.fn();
		render(
			<Toast
				toast={makeToast({ action: { label: "Retry", onClick } })}
				onDismiss={onDismiss}
			/>,
		);
		fireEvent.click(screen.getByText("Retry"));
		expect(onClick).toHaveBeenCalled();
		expect(onDismiss).toHaveBeenCalledWith("t-1");
	});
});
