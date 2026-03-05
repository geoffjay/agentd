/**
 * ServiceConfig — per-service URL configuration with health test buttons.
 */

import { useState } from 'react'
import type { Settings } from '@/stores/settingsStore'

interface ServiceRow {
  key: keyof Settings['services']
  label: string
  port: number
}

const SERVICE_ROWS: ServiceRow[] = [
  { key: 'orchestratorUrl', label: 'Orchestrator', port: 17006 },
  { key: 'notifyUrl', label: 'Notify', port: 17004 },
  { key: 'askUrl', label: 'Ask', port: 17001 },
]

type TestStatus = 'idle' | 'loading' | 'success' | 'error'

interface ServiceConfigProps {
  services: Settings['services']
  onSave: (services: Settings['services']) => void
}

export function ServiceConfig({ services, onSave }: ServiceConfigProps) {
  const [localServices, setLocalServices] = useState<Settings['services']>(services)
  const [testStatuses, setTestStatuses] = useState<Record<keyof Settings['services'], TestStatus>>({
    orchestratorUrl: 'idle',
    notifyUrl: 'idle',
    askUrl: 'idle',
  })

  function handleUrlChange(key: keyof Settings['services'], value: string) {
    setLocalServices(prev => ({ ...prev, [key]: value }))
    setTestStatuses(prev => ({ ...prev, [key]: 'idle' }))
  }

  async function handleTest(key: keyof Settings['services']) {
    const url = localServices[key]
    setTestStatuses(prev => ({ ...prev, [key]: 'loading' }))
    try {
      const response = await fetch(`${url}/health`)
      setTestStatuses(prev => ({
        ...prev,
        [key]: response.ok ? 'success' : 'error',
      }))
    } catch {
      setTestStatuses(prev => ({ ...prev, [key]: 'error' }))
    }
  }

  function handleSave() {
    onSave(localServices)
  }

  return (
    <div className="space-y-4">
      {SERVICE_ROWS.map(row => {
        const status = testStatuses[row.key]
        return (
          <div key={row.key} className="flex items-center gap-3">
            <label
              htmlFor={`service-url-${row.key}`}
              className="w-32 shrink-0 text-sm font-medium text-gray-700 dark:text-gray-300"
            >
              {row.label}
              <span className="block text-xs font-normal text-gray-400 dark:text-gray-500">
                Port {row.port}
              </span>
            </label>
            <input
              id={`service-url-${row.key}`}
              type="text"
              value={localServices[row.key]}
              onChange={e => handleUrlChange(row.key, e.target.value)}
              className="min-w-0 flex-1 rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 placeholder-gray-400 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-800 dark:text-white dark:placeholder-gray-500"
              placeholder={`http://localhost:${row.port}`}
            />
            <button
              type="button"
              onClick={() => void handleTest(row.key)}
              disabled={status === 'loading'}
              aria-label={`Test ${row.label} connection`}
              className="inline-flex items-center gap-1.5 rounded-md border border-gray-300 bg-white px-3 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-60 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700"
            >
              {status === 'loading' ? (
                <span
                  className="inline-block h-4 w-4 animate-spin rounded-full border-2 border-gray-300 border-t-gray-600 dark:border-gray-600 dark:border-t-gray-300"
                  aria-hidden="true"
                />
              ) : status === 'success' ? (
                <span className="text-green-600 dark:text-green-400" aria-hidden="true">✓</span>
              ) : status === 'error' ? (
                <span className="text-red-500 dark:text-red-400" aria-hidden="true">✗</span>
              ) : null}
              Test
            </button>
          </div>
        )
      })}

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

export default ServiceConfig
