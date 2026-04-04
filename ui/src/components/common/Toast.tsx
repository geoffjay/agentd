/**
 * Toast — individual toast notification card.
 *
 * Shows:
 * - Colour-coded left border and icon by type (success/error/warning/info)
 * - Title and optional message
 * - Optional action button
 * - Manual dismiss (X) button
 * - Auto-dismiss progress bar when duration > 0
 */

import {
	AlertCircle,
	AlertTriangle,
	CheckCircle2,
	Info,
	X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { Toast as ToastData } from "@/stores/toastStore";

// ---------------------------------------------------------------------------
// Styling maps
// ---------------------------------------------------------------------------

const BORDER: Record<string, string> = {
	success: "border-l-th-status-success-dot",
	error: "border-l-th-status-error-dot",
	warning: "border-l-th-status-warning-dot",
	info: "border-l-th-status-info-dot",
};

const ICON_CLASS: Record<string, string> = {
	success: "text-th-status-success-text",
	error: "text-th-status-error-text",
	warning: "text-th-status-warning-text",
	info: "text-th-status-info-text",
};

const PROGRESS_CLASS: Record<string, string> = {
	success: "bg-th-status-success-dot",
	error: "bg-th-status-error-dot",
	warning: "bg-th-status-warning-dot",
	info: "bg-th-status-info-dot",
};

function ToastIcon({ type }: { type: string }) {
	const cls = ["shrink-0", ICON_CLASS[type] ?? "text-th-text-muted"].join(" ");
	switch (type) {
		case "success":
			return <CheckCircle2 size={18} className={cls} aria-hidden="true" />;
		case "error":
			return <AlertCircle size={18} className={cls} aria-hidden="true" />;
		case "warning":
			return <AlertTriangle size={18} className={cls} aria-hidden="true" />;
		default:
			return <Info size={18} className={cls} aria-hidden="true" />;
	}
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface ToastProps {
	toast: ToastData;
	onDismiss: (id: string) => void;
}

export function Toast({ toast, onDismiss }: ToastProps) {
	const { id, type, title, message, duration, action } = toast;
	const [progress, setProgress] = useState(100);
	const startRef = useRef(Date.now());
	const frameRef = useRef<number | undefined>(undefined);

	// Auto-dismiss with animated progress bar
	useEffect(() => {
		if (!duration) return;

		const tick = () => {
			const elapsed = Date.now() - startRef.current;
			const remaining = Math.max(0, 100 - (elapsed / duration) * 100);
			setProgress(remaining);
			if (remaining > 0) {
				frameRef.current = requestAnimationFrame(tick);
			} else {
				onDismiss(id);
			}
		};

		frameRef.current = requestAnimationFrame(tick);
		return () => {
			if (frameRef.current) cancelAnimationFrame(frameRef.current);
		};
	}, [id, duration, onDismiss]);

	return (
		<div
			role="alert"
			aria-live={type === "error" ? "assertive" : "polite"}
			aria-atomic="true"
			className={[
				"relative overflow-hidden rounded-lg border border-th-border bg-th-surface shadow-xl",
				"border-l-4",
				BORDER[type] ?? "border-l-th-text-muted",
			].join(" ")}
		>
			<div className="flex items-start gap-3 p-4">
				<ToastIcon type={type} />

				<div className="min-w-0 flex-1">
					<p className="text-sm font-semibold text-th-text">{title}</p>
					{message && (
						<p className="mt-0.5 text-xs text-th-text-muted break-words">
							{message}
						</p>
					)}
					{action && (
						<button
							type="button"
							onClick={() => {
								action.onClick();
								onDismiss(id);
							}}
							className="mt-2 text-xs font-medium text-th-text-link hover:opacity-80 transition-colors"
						>
							{action.label}
						</button>
					)}
				</div>

				<button
					type="button"
					aria-label="Dismiss notification"
					onClick={() => onDismiss(id)}
					className="shrink-0 rounded-md p-0.5 text-th-text-muted hover:bg-th-surface-hover hover:text-th-text transition-colors"
				>
					<X size={14} />
				</button>
			</div>

			{/* Progress bar */}
			{duration > 0 && (
				<div
					className={[
						"absolute bottom-0 left-0 h-0.5 transition-none",
						PROGRESS_CLASS[type] ?? "bg-th-text-muted",
					].join(" ")}
					style={{ width: `${progress}%` }}
					aria-hidden="true"
				/>
			)}
		</div>
	);
}

export default Toast;
