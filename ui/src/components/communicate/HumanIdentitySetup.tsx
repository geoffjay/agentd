/**
 * HumanIdentitySetup — first-time setup modal for human participant identity.
 *
 * Prompts the user for a display name and a unique identifier before they can
 * participate in communicate rooms. The values are persisted in localStorage.
 */

import { useEffect, useRef, useState } from 'react'
import { User, X } from 'lucide-react'
import { FocusTrap } from '@/components/common/FocusTrap'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface HumanIdentitySetupProps {
  open: boolean
  onSave: (identifier: string, displayName: string) => void
  /**
   * When provided the dialog is dismissible (close button + Escape key).
   * Omit when this is a mandatory first-time setup — the user must complete
   * identity configuration before continuing.
   */
  onClose?: () => void
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function HumanIdentitySetup({ open, onSave, onClose }: HumanIdentitySetupProps) {
  const [displayName, setDisplayName] = useState('')
  const [identifier, setIdentifier] = useState('')
  const [errors, setErrors] = useState<{ displayName?: string; identifier?: string }>({})
  const nameRef = useRef<HTMLInputElement>(null)

  // Auto-generate identifier from display name
  useEffect(() => {
    if (displayName && !identifier) {
      setIdentifier(
        'human-' +
          displayName
            .toLowerCase()
            .replace(/\s+/g, '-')
            .replace(/[^a-z0-9-]/g, '')
            .slice(0, 24),
      )
    }
  }, [displayName, identifier])

  // Focus name input when opened
  useEffect(() => {
    if (open) {
      setTimeout(() => nameRef.current?.focus(), 50)
    }
  }, [open])

  if (!open) return null

  function validate(): boolean {
    const e: { displayName?: string; identifier?: string } = {}
    if (!displayName.trim()) e.displayName = 'Display name is required'
    if (!identifier.trim()) e.identifier = 'Identifier is required'
    else if (!/^[a-z0-9][a-z0-9-]*$/.test(identifier.trim()))
      e.identifier = 'Use lowercase letters, numbers, and hyphens only'
    setErrors(e)
    return Object.keys(e).length === 0
  }

  function handleSave() {
    if (!validate()) return
    onSave(identifier.trim(), displayName.trim())
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'Enter') handleSave()
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      {/* Backdrop — clickable only when dismissible */}
      <div
        className="absolute inset-0 bg-black/60"
        aria-hidden="true"
        onClick={onClose}
      />
      <FocusTrap active onEscape={onClose}>
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="identity-setup-title"
          className="relative z-10 w-full max-w-sm rounded-xl bg-gray-800 shadow-xl border border-gray-700"
        >
          {/* Header */}
          <div className="relative flex flex-col items-center px-6 py-6 text-center border-b border-gray-700">
            {/* Close button — only rendered when dismissible */}
            {onClose && (
              <button
                type="button"
                onClick={onClose}
                aria-label="Close dialog"
                className="absolute right-3 top-3 rounded p-1 text-gray-400 hover:text-gray-200 transition-colors"
              >
                <X size={16} />
              </button>
            )}
            <div className="flex h-12 w-12 items-center justify-center rounded-full bg-emerald-600 mb-3">
              <User size={24} className="text-white" />
            </div>
            <h2 id="identity-setup-title" className="text-lg font-semibold text-white">
              {onClose ? 'Edit your identity' : 'Set up your identity'}
            </h2>
            <p className="mt-1 text-sm text-gray-400">
              Choose how you appear to agents and other participants.
            </p>
          </div>

          {/* Body */}
          <div className="px-6 py-5 space-y-4" onKeyDown={handleKeyDown}>
            <div>
              <label className="block text-sm font-medium text-gray-300 mb-1">
                Display name <span className="text-red-400">*</span>
              </label>
              <input
                ref={nameRef}
                type="text"
                value={displayName}
                onChange={(e) => setDisplayName(e.target.value)}
                placeholder="e.g. Alice"
                className={inputClass(errors.displayName)}
              />
              {errors.displayName && (
                <p className="mt-1 text-xs text-red-400">{errors.displayName}</p>
              )}
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-300 mb-1">
                Identifier <span className="text-red-400">*</span>
              </label>
              <input
                type="text"
                value={identifier}
                onChange={(e) => setIdentifier(e.target.value)}
                placeholder="e.g. human-alice"
                className={inputClass(errors.identifier)}
              />
              {errors.identifier ? (
                <p className="mt-1 text-xs text-red-400">{errors.identifier}</p>
              ) : (
                <p className="mt-1 text-xs text-gray-500">
                  Unique ID used by agents to address you. Lowercase letters, numbers, and hyphens.
                </p>
              )}
            </div>
          </div>

          {/* Footer */}
          <div className="flex justify-end px-6 py-4 border-t border-gray-700">
            <button
              type="button"
              onClick={handleSave}
              className="rounded-md bg-emerald-600 px-5 py-2 text-sm font-medium text-white hover:bg-emerald-700 transition-colors"
            >
              Get started
            </button>
          </div>
        </div>
      </FocusTrap>
    </div>
  )
}

function inputClass(error?: string): string {
  return [
    'w-full rounded-md border px-3 py-2 text-sm bg-gray-900 text-white',
    'placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-primary-500',
    error ? 'border-red-500' : 'border-gray-600',
  ].join(' ')
}
