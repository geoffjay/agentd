/**
 * UIPreferences — UI preferences form for theme, sidebar, refresh interval,
 * notifications, and log view lines.
 *
 * Theme changes apply immediately to the DOM via ThemeContext so users
 * can preview the effect before saving.
 */

import { useState } from 'react'
import type { Settings } from '@/stores/settingsStore'
import { useTheme } from '@/hooks/useTheme'

interface UIPreferencesProps {
  ui: Settings['ui']
  onSave: (ui: Settings['ui']) => void
}

export function UIPreferences({ ui, onSave }: UIPreferencesProps) {
  const [localUI, setLocalUI] = useState<Settings['ui']>(ui)
  const { setTheme } = useTheme()

  function handleSave() {
    onSave(localUI)
  }

  return (
    <div className="space-y-5">
      {/* Theme */}
      <div className="flex items-center justify-between">
        <label
          htmlFor="ui-theme"
          className="text-sm font-medium text-gray-700 dark:text-gray-300"
        >
          Theme
        </label>
        <select
          id="ui-theme"
          value={localUI.theme}
          onChange={e => {
            const theme = e.target.value as Settings['ui']['theme']
            setLocalUI(prev => ({ ...prev, theme }))
            // Apply immediately so the user sees the change in real time
            setTheme(theme)
          }}
          className="rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-800 dark:text-white"
        >
          <option value="light">Light</option>
          <option value="dark">Dark</option>
          <option value="system">System</option>
        </select>
      </div>

      {/* Sidebar default open */}
      <div className="flex items-center justify-between">
        <label
          htmlFor="ui-sidebar-open"
          className="text-sm font-medium text-gray-700 dark:text-gray-300"
        >
          Sidebar
        </label>
        <label className="flex cursor-pointer items-center gap-2 text-sm text-gray-600 dark:text-gray-400">
          <input
            id="ui-sidebar-open"
            type="checkbox"
            checked={localUI.sidebarDefaultOpen}
            onChange={e =>
              setLocalUI(prev => ({ ...prev, sidebarDefaultOpen: e.target.checked }))
            }
            className="h-4 w-4 rounded border-gray-300 text-primary-600 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-800"
          />
          Open by default
        </label>
      </div>

      {/* Refresh interval */}
      <div className="flex items-center justify-between">
        <label
          htmlFor="ui-refresh-interval"
          className="text-sm font-medium text-gray-700 dark:text-gray-300"
        >
          Refresh interval
        </label>
        <select
          id="ui-refresh-interval"
          value={localUI.refreshInterval}
          onChange={e =>
            setLocalUI(prev => ({
              ...prev,
              refreshInterval: Number(e.target.value) as Settings['ui']['refreshInterval'],
            }))
          }
          className="rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-800 dark:text-white"
        >
          <option value={30}>30s</option>
          <option value={60}>60s</option>
          <option value={120}>2m</option>
          <option value={300}>5m</option>
        </select>
      </div>

      {/* Notifications */}
      <div className="flex items-center justify-between">
        <label
          htmlFor="ui-notifications"
          className="text-sm font-medium text-gray-700 dark:text-gray-300"
        >
          Notifications
        </label>
        <label className="flex cursor-pointer items-center gap-2 text-sm text-gray-600 dark:text-gray-400">
          <input
            id="ui-notifications"
            type="checkbox"
            checked={localUI.notificationsEnabled}
            onChange={e =>
              setLocalUI(prev => ({ ...prev, notificationsEnabled: e.target.checked }))
            }
            className="h-4 w-4 rounded border-gray-300 text-primary-600 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-800"
          />
          Enable desktop notifications
        </label>
      </div>

      {/* Log view lines */}
      <div className="flex items-center justify-between">
        <label
          htmlFor="ui-log-lines"
          className="text-sm font-medium text-gray-700 dark:text-gray-300"
        >
          Log view lines
        </label>
        <select
          id="ui-log-lines"
          value={localUI.logViewLines}
          onChange={e =>
            setLocalUI(prev => ({
              ...prev,
              logViewLines: Number(e.target.value) as Settings['ui']['logViewLines'],
            }))
          }
          className="rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-800 dark:text-white"
        >
          <option value={100}>100</option>
          <option value={250}>250</option>
          <option value={500}>500</option>
          <option value={1000}>1000</option>
        </select>
      </div>

      <div className="pt-2">
        <button
          type="button"
          onClick={handleSave}
          className="rounded-md bg-primary-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-primary-700 focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2 dark:focus:ring-offset-gray-900"
        >
          Save
        </button>
      </div>
    </div>
  )
}

export default UIPreferences
