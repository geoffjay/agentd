/**
 * SettingsPage — assembles all settings sections including service config,
 * UI preferences, about info, and data management actions.
 */

import { useRef, useState } from 'react'
import { ServiceConfig } from '@/components/settings/ServiceConfig'
import { UIPreferences } from '@/components/settings/UIPreferences'
import { AboutSection } from '@/components/settings/AboutSection'
import { PipelineGates } from '@/components/settings/PipelineGates'
import { useSettings } from '@/hooks/useSettings'
import { resetSettings } from '@/stores/settingsStore'
import type { Settings } from '@/stores/settingsStore'

export function SettingsPage() {
  const { settings, updateServices, updateUI, reset } = useSettings()
  const [clearConfirmed, setClearConfirmed] = useState(false)
  const importInputRef = useRef<HTMLInputElement>(null)

  function handleClearAll() {
    if (!clearConfirmed) {
      setClearConfirmed(true)
      return
    }
    resetSettings()
    reset()
    setClearConfirmed(false)
  }

  function handleExport() {
    const blob = new Blob([JSON.stringify(settings, null, 2)], {
      type: 'application/json',
    })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = 'agentd-settings.json'
    a.click()
    URL.revokeObjectURL(url)
  }

  function handleImportClick() {
    importInputRef.current?.click()
  }

  function handleImportFile(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0]
    if (!file) return
    const reader = new FileReader()
    reader.onload = (event) => {
      try {
        const parsed = JSON.parse(event.target?.result as string) as Partial<Settings>
        if (parsed.services) updateServices(parsed.services)
        if (parsed.ui) updateUI(parsed.ui)
      } catch {
        // silently ignore malformed JSON
      }
    }
    reader.readAsText(file)
    // Reset input so the same file can be re-imported
    e.target.value = ''
  }

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-2xl font-semibold text-gray-900 dark:text-white">Settings</h1>
        <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
          Manage service connections, UI preferences, and application data.
        </p>
      </div>

      {/* Service Configuration */}
      <section className="rounded-lg border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-white">
          Service Configuration
        </h2>
        <ServiceConfig services={settings.services} onSave={updateServices} />
      </section>

      {/* UI Preferences */}
      <section className="rounded-lg border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-white">UI Preferences</h2>
        <UIPreferences ui={settings.ui} onSave={updateUI} />
      </section>

      {/* Pipeline Gates */}
      <section className="rounded-lg border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <h2 className="mb-1 text-lg font-semibold text-gray-900 dark:text-white">
          Autonomous Pipeline Gates
        </h2>
        <p className="mb-4 text-sm text-gray-500 dark:text-gray-400">
          v0.10.0 — defines where human approval is required in the autonomous pipeline.
        </p>
        <PipelineGates />
      </section>

      {/* About */}
      <section className="rounded-lg border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-white">About</h2>
        <AboutSection />
      </section>

      {/* Data Management */}
      <section className="rounded-lg border border-gray-200 bg-white p-6 shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-white">
          Data Management
        </h2>
        <div className="flex flex-wrap gap-3">
          <button
            type="button"
            onClick={handleClearAll}
            className={`rounded-md px-4 py-2 text-sm font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-offset-2 dark:focus:ring-offset-gray-900 ${
              clearConfirmed
                ? 'bg-red-600 text-white hover:bg-red-700 focus:ring-red-500'
                : 'border border-red-300 text-red-600 hover:bg-red-50 focus:ring-red-500 dark:border-red-700 dark:text-red-400 dark:hover:bg-red-900/20'
            }`}
          >
            {clearConfirmed ? 'Confirm Clear All Settings' : 'Clear All Settings'}
          </button>

          <button
            type="button"
            onClick={handleExport}
            className="rounded-md border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700 dark:focus:ring-offset-gray-900"
          >
            Export Settings
          </button>

          <button
            type="button"
            onClick={handleImportClick}
            className="rounded-md border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700 dark:focus:ring-offset-gray-900"
          >
            Import Settings
          </button>
          <input
            ref={importInputRef}
            type="file"
            accept=".json"
            className="hidden"
            onChange={handleImportFile}
            aria-label="Import settings file"
          />
        </div>
      </section>
    </div>
  )
}

export default SettingsPage
