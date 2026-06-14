/**
 * Generic data-fetching hook for the paginated, read-only product-admin lists.
 *
 * Pass a loader that returns a `PaginatedResponse<T>`; the hook manages
 * loading/error state and offset-based pagination. The loader is held in a ref
 * so an inline arrow function (new identity each render) does not retrigger the
 * fetch — only an offset change or an explicit `refetch()` does.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import type { PaginatedResponse } from "@/types/common";

export const ADMIN_PAGE_SIZE = 50;

interface AdminResourceParams {
	limit: number;
	offset: number;
}

export interface UseAdminResourceResult<T> {
	items: T[];
	total: number;
	offset: number;
	limit: number;
	loading: boolean;
	error?: string;
	setOffset: (offset: number) => void;
	refetch: () => void;
}

export function useAdminResource<T>(
	loader: (params: AdminResourceParams) => Promise<PaginatedResponse<T>>,
): UseAdminResourceResult<T> {
	const loaderRef = useRef(loader);
	loaderRef.current = loader;

	const [items, setItems] = useState<T[]>([]);
	const [total, setTotal] = useState(0);
	const [offset, setOffset] = useState(0);
	const [loading, setLoading] = useState(true);
	const [error, setError] = useState<string | undefined>(undefined);

	const load = useCallback(async (off: number) => {
		setLoading(true);
		setError(undefined);
		try {
			const page = await loaderRef.current({
				limit: ADMIN_PAGE_SIZE,
				offset: off,
			});
			setItems(page.items);
			setTotal(page.total);
		} catch (err) {
			setError(err instanceof Error ? err.message : "Failed to load");
		} finally {
			setLoading(false);
		}
	}, []);

	useEffect(() => {
		void load(offset);
	}, [load, offset]);

	return {
		items,
		total,
		offset,
		limit: ADMIN_PAGE_SIZE,
		loading,
		error,
		setOffset,
		refetch: () => load(offset),
	};
}
