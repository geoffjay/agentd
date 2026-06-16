/**
 * DocumentEditor — CodeMirror 6 markdown editor with debounced autosave.
 *
 * - Creates an EditorView imperatively in a useRef div.
 * - Debounces changes (1 500 ms) and calls updateDocument with the optimistic-
 *   concurrency precondition (expected_updated_at = last known updated_at).
 * - Tears down the editor on unmount.
 */

import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import { oneDark } from "@codemirror/theme-one-dark";
import { EditorState } from "@codemirror/state";
import { EditorView, keymap, lineNumbers } from "@codemirror/view";
import { useEffect, useRef } from "react";
import type { DocumentContent } from "@/types/knowledge";

interface DocumentEditorProps {
	docContent: DocumentContent | null;
	/** Called whenever the editor content changes (debounced). */
	onSave: (content: string, expectedUpdatedAt: string) => void;
	onSavingChange: (saving: boolean) => void;
}

const DEBOUNCE_MS = 1_500;

export function DocumentEditor({
	docContent,
	onSave,
	onSavingChange,
}: DocumentEditorProps) {
	const containerRef = useRef<HTMLDivElement>(null);
	const viewRef = useRef<EditorView | null>(null);
	const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
	// Track the last-known updated_at for optimistic concurrency
	const updatedAtRef = useRef<string>("");

	// Recreate the editor whenever the document changes
	useEffect(() => {
		if (!containerRef.current) return;

		// Destroy previous view
		viewRef.current?.destroy();
		if (debounceRef.current) clearTimeout(debounceRef.current);

		if (!docContent) {
			viewRef.current = null;
			return;
		}

		updatedAtRef.current = docContent.document.updated_at;
		const initialContent = docContent.content;

		const updateListener = EditorView.updateListener.of((update) => {
			if (!update.docChanged) return;

			onSavingChange(true);

			if (debounceRef.current) clearTimeout(debounceRef.current);
			debounceRef.current = setTimeout(() => {
				const content = update.state.doc.toString();
				onSave(content, updatedAtRef.current);
			}, DEBOUNCE_MS);
		});

		const state = EditorState.create({
			doc: initialContent,
			extensions: [
				history(),
				keymap.of([...defaultKeymap, ...historyKeymap]),
				lineNumbers(),
				markdown(),
				oneDark,
				updateListener,
				EditorView.theme({
					"&": { height: "100%", fontSize: "13px" },
					".cm-scroller": { overflow: "auto", fontFamily: "monospace" },
				}),
			],
		});

		viewRef.current = new EditorView({
			state,
			parent: containerRef.current,
		});

		return () => {
			if (debounceRef.current) clearTimeout(debounceRef.current);
			viewRef.current?.destroy();
			viewRef.current = null;
		};
	}, [docContent?.document.id]); // recreate only when the doc ID changes

	// Update the updatedAt ref whenever the server confirms a save
	useEffect(() => {
		if (docContent) {
			updatedAtRef.current = docContent.document.updated_at;
		}
	}, [docContent?.document.updated_at]);

	if (!docContent) {
		return (
			<div className="flex h-full items-center justify-center text-sm text-th-text-muted">
				Select a document from the tree to edit it.
			</div>
		);
	}

	return (
		<div
			ref={containerRef}
			className="h-full w-full overflow-hidden"
			aria-label="Markdown editor"
		/>
	);
}
