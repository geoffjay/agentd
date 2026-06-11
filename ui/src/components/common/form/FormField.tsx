/**
 * FormField — standard wrapper for a labeled form control.
 *
 * Renders a label (with optional "(optional)" hint), the control itself,
 * help text, and a validation error message in the shared theme styles.
 */

import type { ReactNode } from "react";

export interface FormFieldProps {
	htmlFor?: string;
	label: string;
	optional?: boolean;
	help?: string;
	error?: string;
	children: ReactNode;
}

/** Shared input class string used by form controls across the app. */
export function fieldClass(error?: string, extra = ""): string {
	return [
		"block w-full rounded-md border px-3 py-2 text-sm",
		"bg-th-input text-th-text placeholder:text-th-text-faint",
		"focus:outline-none focus:ring-2 focus:ring-th-focus-ring",
		"disabled:cursor-not-allowed disabled:opacity-50",
		error ? "border-th-status-error-border" : "border-th-border-input",
		extra,
	]
		.filter(Boolean)
		.join(" ");
}

export function FormField({
	htmlFor,
	label,
	optional,
	help,
	error,
	children,
}: FormFieldProps) {
	return (
		<div>
			<label
				htmlFor={htmlFor}
				className="mb-1 block text-sm font-medium text-th-text-secondary"
			>
				{label}
				{optional && (
					<span className="ml-1 text-xs font-normal text-th-text-faint">
						(optional)
					</span>
				)}
			</label>
			{children}
			{help && !error && (
				<p className="mt-1 text-xs text-th-text-faint">{help}</p>
			)}
			{error && (
				<p className="mt-1 text-xs text-th-status-error-text">{error}</p>
			)}
		</div>
	);
}

export default FormField;
