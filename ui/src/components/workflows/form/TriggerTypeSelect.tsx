/**
 * TriggerTypeSelect — grouped dropdown of all trigger types.
 *
 * Groups options by the registry's group field (Polling / Schedule /
 * Events / Advanced) and shows the selected type's description below.
 */

import { FormField, fieldClass } from "@/components/common/form";
import type { TriggerType } from "@/types/orchestrator";
import { TRIGGER_DEFS, type TriggerGroup, triggerDef } from "./triggerDefs";

export interface TriggerTypeSelectProps {
	value: TriggerType;
	onChange: (type: TriggerType) => void;
	disabled?: boolean;
	/** Hide the composite option (used at the nesting depth cap). */
	excludeComposite?: boolean;
	id?: string;
}

const GROUP_ORDER: TriggerGroup[] = [
	"Polling",
	"Schedule",
	"Events",
	"Advanced",
];

export function TriggerTypeSelect({
	value,
	onChange,
	disabled,
	excludeComposite,
	id = "wf-trigger-type",
}: TriggerTypeSelectProps) {
	const defs = excludeComposite
		? TRIGGER_DEFS.filter((d) => d.type !== "composite")
		: TRIGGER_DEFS;

	return (
		<FormField
			htmlFor={id}
			label="Trigger type"
			help={triggerDef(value).description}
		>
			<select
				id={id}
				value={value}
				onChange={(e) => onChange(e.target.value as TriggerType)}
				disabled={disabled}
				className={fieldClass()}
			>
				{GROUP_ORDER.map((group) => {
					const grouped = defs.filter((d) => d.group === group);
					if (grouped.length === 0) return null;
					return (
						<optgroup key={group} label={group}>
							{grouped.map((d) => (
								<option key={d.type} value={d.type}>
									{d.label}
								</option>
							))}
						</optgroup>
					);
				})}
			</select>
		</FormField>
	);
}

export default TriggerTypeSelect;
