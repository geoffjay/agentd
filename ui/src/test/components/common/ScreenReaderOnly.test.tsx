/**
 * ScreenReaderOnly -- unit tests.
 */

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ScreenReaderOnly } from "@/components/common/ScreenReaderOnly";

describe("ScreenReaderOnly", () => {
	it("renders children text", () => {
		render(<ScreenReaderOnly>Hidden label</ScreenReaderOnly>);
		expect(screen.getByText("Hidden label")).toBeInTheDocument();
	});

	it("defaults to a span element", () => {
		render(<ScreenReaderOnly>test</ScreenReaderOnly>);
		const el = screen.getByText("test");
		expect(el.tagName).toBe("SPAN");
	});

	it("renders as a different element when specified", () => {
		render(<ScreenReaderOnly as="h2">Heading</ScreenReaderOnly>);
		const el = screen.getByText("Heading");
		expect(el.tagName).toBe("H2");
	});

	it("applies sr-only styles", () => {
		render(<ScreenReaderOnly>test</ScreenReaderOnly>);
		const el = screen.getByText("test");
		expect(el.className).toContain("absolute");
		expect(el.className).toContain("overflow-hidden");
	});
});
