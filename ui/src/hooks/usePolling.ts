/**
 * usePolling -- shared interval polling helper for dashboard data hooks.
 *
 * - Fires the callback immediately on mount.
 * - Re-fires every `intervalMs` (default 30s).
 * - Pauses while the document is hidden and resumes (with an immediate
 *   fire) when it becomes visible again.
 * - Cleans up the interval and visibility listener on unmount.
 */

import { useEffect, useRef } from "react";

export const DEFAULT_POLL_INTERVAL_MS = 30_000;

export function usePolling(
	callback: () => void | Promise<void>,
	intervalMs: number = DEFAULT_POLL_INTERVAL_MS,
): void {
	const callbackRef = useRef(callback);

	useEffect(() => {
		callbackRef.current = callback;
	}, [callback]);

	useEffect(() => {
		let intervalId: ReturnType<typeof setInterval> | null = null;

		const tick = () => {
			void callbackRef.current();
		};

		const start = () => {
			if (intervalId === null) {
				intervalId = setInterval(tick, intervalMs);
			}
		};

		const stop = () => {
			if (intervalId !== null) {
				clearInterval(intervalId);
				intervalId = null;
			}
		};

		const handleVisibilityChange = () => {
			if (document.hidden) {
				stop();
			} else {
				tick();
				start();
			}
		};

		tick();
		if (!document.hidden) start();
		document.addEventListener("visibilitychange", handleVisibilityChange);

		return () => {
			stop();
			document.removeEventListener("visibilitychange", handleVisibilityChange);
		};
	}, [intervalMs]);
}
