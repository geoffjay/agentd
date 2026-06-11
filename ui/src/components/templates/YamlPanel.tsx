/**
 * YamlPanel — YAML import/export side panel for the agent and workflow
 * form pages.
 *
 * Shows a live template preview of the current form state (with a copy
 * button), and an import area accepting pasted YAML or an uploaded
 * `.yml` file. Warnings from either direction render beneath.
 */

import { AlertTriangle, Check, Copy, Upload } from "lucide-react";
import { type ChangeEvent, useRef, useState } from "react";
import { HighlightedCode } from "@/components/common";
import { fieldClass } from "@/components/common/form";

export interface YamlPanelProps {
	/** Live export of the current form state. */
	exportedYaml: string;
	/** Warnings from the live export (e.g. redacted env values). */
	exportWarnings: string[];
	/**
	 * Import handler: parse the YAML and replace the form state.
	 * Returns the warnings to display; throws on parse errors.
	 */
	onImport: (text: string) => string[];
	disabled?: boolean;
	/** Panel title, e.g. "Agent template". */
	title: string;
}

export function YamlPanel({
	exportedYaml,
	exportWarnings,
	onImport,
	disabled,
	title,
}: YamlPanelProps) {
	const [importText, setImportText] = useState("");
	const [importWarnings, setImportWarnings] = useState<string[]>([]);
	const [importError, setImportError] = useState<string | undefined>();
	const [copied, setCopied] = useState(false);
	const fileInputRef = useRef<HTMLInputElement>(null);

	function runImport(text: string) {
		setImportError(undefined);
		try {
			setImportWarnings(onImport(text));
		} catch (err) {
			setImportWarnings([]);
			setImportError(
				err instanceof Error ? err.message : "Failed to parse YAML",
			);
		}
	}

	function handleFile(e: ChangeEvent<HTMLInputElement>) {
		const file = e.target.files?.[0];
		if (!file) return;
		const reader = new FileReader();
		reader.onload = () => {
			const text = String(reader.result ?? "");
			setImportText(text);
			runImport(text);
		};
		reader.readAsText(file);
		e.target.value = "";
	}

	async function copyExport() {
		await navigator.clipboard.writeText(exportedYaml);
		setCopied(true);
		setTimeout(() => setCopied(false), 1500);
	}

	const warnings = [...exportWarnings, ...importWarnings];

	return (
		<aside className="space-y-4 rounded-lg border border-th-border bg-th-surface p-5">
			<div className="flex items-center justify-between">
				<h2 className="text-sm font-semibold text-th-text">{title}</h2>
				<button
					type="button"
					onClick={copyExport}
					className="inline-flex items-center gap-1.5 rounded-md border border-th-border-strong px-2.5 py-1.5 text-xs font-medium text-th-text-secondary hover:bg-th-surface-hover"
				>
					{copied ? <Check size={12} /> : <Copy size={12} />}
					{copied ? "Copied" : "Copy YAML"}
				</button>
			</div>

			<p className="text-xs text-th-text-faint">
				Live preview of this form as an .agentd template — save it for use with{" "}
				<code className="font-mono">agent apply</code>.
			</p>

			<HighlightedCode
				code={exportedYaml}
				language="yaml"
				maxHeight="20rem"
				className="border border-th-border"
			/>

			{warnings.length > 0 && (
				<ul className="space-y-1.5 rounded-md border border-th-status-warning-border bg-th-status-warning-bg px-3 py-2">
					{warnings.map((warning) => (
						<li
							key={warning}
							className="flex items-start gap-2 text-xs text-th-status-warning-text"
						>
							<AlertTriangle
								size={12}
								className="mt-0.5 shrink-0"
								aria-hidden="true"
							/>
							{warning}
						</li>
					))}
				</ul>
			)}

			<div className="space-y-2 border-t border-th-border pt-4">
				<div className="flex items-center justify-between">
					<span className="text-xs font-medium text-th-text-secondary">
						Import a template
					</span>
					<button
						type="button"
						onClick={() => fileInputRef.current?.click()}
						disabled={disabled}
						className="inline-flex items-center gap-1.5 rounded-md border border-th-border-strong px-2.5 py-1.5 text-xs font-medium text-th-text-secondary hover:bg-th-surface-hover disabled:opacity-50"
					>
						<Upload size={12} />
						Upload file
					</button>
					<input
						ref={fileInputRef}
						type="file"
						accept=".yml,.yaml,text/yaml"
						onChange={handleFile}
						className="hidden"
						aria-label="Upload YAML template"
					/>
				</div>

				<textarea
					value={importText}
					onChange={(e) => setImportText(e.target.value)}
					rows={6}
					placeholder="Paste an .agentd YAML template…"
					disabled={disabled}
					aria-label="YAML template to import"
					className={fieldClass(importError, "font-mono text-xs")}
				/>
				{importError && (
					<p className="text-xs text-th-status-error-text">{importError}</p>
				)}

				<button
					type="button"
					onClick={() => runImport(importText)}
					disabled={disabled || !importText.trim()}
					className="rounded-md bg-th-accent px-3 py-1.5 text-xs font-medium text-th-accent-text hover:bg-th-accent-hover disabled:opacity-50"
				>
					Import into form
				</button>
			</div>
		</aside>
	);
}

export default YamlPanel;
