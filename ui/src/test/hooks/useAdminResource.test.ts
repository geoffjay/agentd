/**
 * useAdminResource — the data-fetching hook behind every product-admin list.
 *
 * Verifies the initial fetch, error capture, offset-driven refetch, the stable
 * loader ref (a new loader identity must not retrigger a fetch), and refetch().
 */

import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ADMIN_PAGE_SIZE, useAdminResource } from "@/hooks/useAdminResource";
import type { PaginatedResponse } from "@/types/common";

function page<T>(items: T[], total = items.length): PaginatedResponse<T> {
	return { items, total, limit: ADMIN_PAGE_SIZE, offset: 0 };
}

describe("useAdminResource", () => {
	it("loads the first page on mount and exposes the page size", async () => {
		const loader = vi.fn(async () => page([{ id: "a" }], 1));
		const { result } = renderHook(() => useAdminResource(loader));

		// Starts in a loading state before the first resolution.
		expect(result.current.loading).toBe(true);

		await waitFor(() => expect(result.current.loading).toBe(false));
		expect(result.current.items).toEqual([{ id: "a" }]);
		expect(result.current.total).toBe(1);
		expect(result.current.limit).toBe(ADMIN_PAGE_SIZE);
		expect(loader).toHaveBeenCalledWith({ limit: ADMIN_PAGE_SIZE, offset: 0 });
	});

	it("captures the error message and clears loading on failure", async () => {
		const loader = vi.fn(async () => {
			throw new Error("nope");
		});
		const { result } = renderHook(() => useAdminResource(loader));

		await waitFor(() => expect(result.current.loading).toBe(false));
		expect(result.current.error).toBe("nope");
		expect(result.current.items).toEqual([]);
	});

	it("uses a fallback message for non-Error rejections", async () => {
		const loader = vi.fn(async () => {
			// eslint-disable-next-line @typescript-eslint/only-throw-error
			throw "boom";
		});
		const { result } = renderHook(() => useAdminResource(loader));

		await waitFor(() => expect(result.current.loading).toBe(false));
		expect(result.current.error).toBe("Failed to load");
	});

	it("refetches with the new offset when setOffset is called", async () => {
		const loader = vi.fn(async ({ offset }: { offset: number }) =>
			page([{ id: `row-${offset}` }], 100),
		);
		const { result } = renderHook(() => useAdminResource(loader));

		await waitFor(() => expect(result.current.loading).toBe(false));

		act(() => result.current.setOffset(ADMIN_PAGE_SIZE));

		await waitFor(() =>
			expect(result.current.items).toEqual([{ id: `row-${ADMIN_PAGE_SIZE}` }]),
		);
		expect(result.current.offset).toBe(ADMIN_PAGE_SIZE);
		expect(loader).toHaveBeenLastCalledWith({
			limit: ADMIN_PAGE_SIZE,
			offset: ADMIN_PAGE_SIZE,
		});
	});

	it("does not refetch when only the loader identity changes", async () => {
		const calls: number[] = [];
		const makeLoader = () =>
			vi.fn(async ({ offset }: { offset: number }) => {
				calls.push(offset);
				return page([{ id: "x" }], 1);
			});

		const { result, rerender } = renderHook(
			({ loader }) => useAdminResource(loader),
			{ initialProps: { loader: makeLoader() } },
		);

		await waitFor(() => expect(result.current.loading).toBe(false));
		expect(calls).toHaveLength(1);

		// New loader function identity on re-render must NOT retrigger a fetch.
		rerender({ loader: makeLoader() });
		await Promise.resolve();
		expect(calls).toHaveLength(1);
	});

	it("re-runs the current page when refetch is called", async () => {
		const loader = vi.fn(async () => page([{ id: "a" }], 1));
		const { result } = renderHook(() => useAdminResource(loader));

		await waitFor(() => expect(result.current.loading).toBe(false));
		expect(loader).toHaveBeenCalledTimes(1);

		await act(async () => {
			result.current.refetch();
		});

		await waitFor(() => expect(loader).toHaveBeenCalledTimes(2));
		expect(loader).toHaveBeenLastCalledWith({
			limit: ADMIN_PAGE_SIZE,
			offset: 0,
		});
	});
});
