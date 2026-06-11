/**
 * ToggleSwitch — accessible on/off switch in the shared theme styles.
 *
 * Extracted from the inline switches previously duplicated in the agent
 * and workflow forms.
 */

export interface ToggleSwitchProps {
	checked: boolean;
	onChange: (value: boolean) => void;
	label: string;
	disabled?: boolean;
}

export function ToggleSwitch({
	checked,
	onChange,
	label,
	disabled,
}: ToggleSwitchProps) {
	return (
		<button
			type="button"
			role="switch"
			aria-checked={checked}
			aria-label={label}
			disabled={disabled}
			onClick={() => onChange(!checked)}
			className={[
				"relative inline-flex h-6 w-11 flex-shrink-0 items-center rounded-full transition-colors",
				"focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-th-focus-ring",
				"disabled:cursor-not-allowed disabled:opacity-50",
				checked ? "bg-th-accent" : "bg-th-surface-sunken",
			].join(" ")}
		>
			<span
				className={[
					"inline-block h-4 w-4 rounded-full bg-th-surface shadow transition-transform",
					checked ? "translate-x-6" : "translate-x-1",
				].join(" ")}
			/>
		</button>
	);
}

export default ToggleSwitch;
