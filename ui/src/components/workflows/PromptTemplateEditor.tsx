/**
 * PromptTemplateEditor — textarea with variable placeholder hints and live preview.
 *
 * Shows available template variables ({{title}}, {{body}}, etc.) and
 * renders a preview substituting sample values so the user can see
 * how the prompt will look when dispatched.
 */

import { useState } from 'react'
import { ChevronDown, ChevronUp, Info } from 'lucide-react'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface PromptTemplateEditorProps {
  value: string
  onChange: (value: string) => void
  disabled?: boolean
  error?: string
}

// ---------------------------------------------------------------------------
// Template variable definitions
// ---------------------------------------------------------------------------

const TEMPLATE_VARS: Array<{ name: string; description: string; sample: string }> = [
  { name: '{{title}}', description: 'Task title', sample: 'Fix login bug' },
  { name: '{{body}}', description: 'Task body / description', sample: 'Users cannot log in with SSO...' },
  { name: '{{url}}', description: 'Source URL', sample: 'https://github.com/owner/repo/issues/42' },
  { name: '{{labels}}', description: 'Comma-separated labels', sample: 'bug, high-priority' },
  { name: '{{source_id}}', description: 'Source identifier (e.g. issue number)', sample: '42' },
]

const DEFAULT_TEMPLATE =
  `You are working on the following task:\n\nTitle: {{title}}\n\nDescription:\n{{body}}\n\nSource: {{url}}\nLabels: {{labels}}\n\nPlease work on this task and report back when complete.`

/** Render a template with sample values for preview */
function renderPreview(template: string): string {
  let result = template
  for (const v of TEMPLATE_VARS) {
    result = result.replaceAll(v.name, v.sample)
  }
  return result
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function PromptTemplateEditor({
  value,
  onChange,
  disabled = false,
  error,
}: PromptTemplateEditorProps) {
  const [showPreview, setShowPreview] = useState(false)
  const [showVars, setShowVars] = useState(false)

  function insertVariable(varName: string) {
    // Append variable at cursor position. As a simple fallback, append at end.
    onChange(value + varName)
  }

  const preview = renderPreview(value || DEFAULT_TEMPLATE)
  const hasValue = value.trim().length > 0

  return (
    <div className="space-y-2">
      {/* Textarea */}
      <textarea
        value={value}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
        rows={6}
        placeholder={DEFAULT_TEMPLATE}
        className={[
          'w-full rounded-md border px-3 py-2 text-sm font-mono',
          'bg-white dark:bg-gray-900',
          'text-gray-900 dark:text-white',
          'placeholder:text-gray-400 dark:placeholder:text-gray-600',
          'focus:outline-none focus:ring-2 focus:ring-primary-500',
          'disabled:cursor-not-allowed disabled:opacity-50',
          error
            ? 'border-red-400 dark:border-red-500'
            : 'border-gray-300 dark:border-gray-600',
        ].join(' ')}
      />
      {error && <p className="text-xs text-red-500 dark:text-red-400">{error}</p>}

      {/* Available variables */}
      <div className="rounded-md border border-gray-200 dark:border-gray-700 overflow-hidden">
        <button
          type="button"
          onClick={() => setShowVars((v) => !v)}
          className="flex w-full items-center justify-between px-3 py-2 text-xs text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
        >
          <span className="flex items-center gap-1.5">
            <Info size={12} />
            Available variables
          </span>
          {showVars ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
        </button>

        {showVars && (
          <div className="border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50 px-3 py-2">
            <div className="flex flex-wrap gap-2">
              {TEMPLATE_VARS.map((v) => (
                <button
                  key={v.name}
                  type="button"
                  onClick={() => insertVariable(v.name)}
                  disabled={disabled}
                  title={`${v.description} — click to insert`}
                  className="inline-flex items-center gap-1 rounded border border-gray-200 dark:border-gray-600 bg-white dark:bg-gray-800 px-2 py-0.5 font-mono text-xs text-primary-600 dark:text-primary-400 hover:bg-primary-50 dark:hover:bg-primary-900/20 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {v.name}
                </button>
              ))}
            </div>
            <table className="mt-2 w-full text-xs">
              <tbody>
                {TEMPLATE_VARS.map((v) => (
                  <tr key={v.name} className="border-t border-gray-100 dark:border-gray-700">
                    <td className="py-1 pr-3 font-mono text-primary-600 dark:text-primary-400 whitespace-nowrap">{v.name}</td>
                    <td className="py-1 pr-3 text-gray-600 dark:text-gray-400">{v.description}</td>
                    <td className="py-1 text-gray-400 dark:text-gray-500 italic">{v.sample}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Preview */}
      {hasValue && (
        <div className="rounded-md border border-gray-200 dark:border-gray-700 overflow-hidden">
          <button
            type="button"
            onClick={() => setShowPreview((v) => !v)}
            className="flex w-full items-center justify-between px-3 py-2 text-xs text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
          >
            <span>Preview with sample data</span>
            {showPreview ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
          </button>

          {showPreview && (
            <pre className="border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50 px-3 py-2 text-xs text-gray-700 dark:text-gray-300 whitespace-pre-wrap font-mono overflow-auto max-h-48">
              {preview}
            </pre>
          )}
        </div>
      )}
    </div>
  )
}

export default PromptTemplateEditor
