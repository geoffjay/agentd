/**
 * settingsStore — versioned localStorage settings store.
 *
 * Key: 'agentd:settings', version: 1
 * Merges with defaults on load so new keys added in future versions
 * are always present.
 */

export interface ServiceSettings {
  orchestratorUrl: string
  notifyUrl: string
  askUrl: string
}

export interface UISettings {
  theme: 'light' | 'dark' | 'system'
  sidebarDefaultOpen: boolean
  refreshInterval: 30 | 60 | 120 | 300
  notificationsEnabled: boolean
  logViewLines: 100 | 250 | 500 | 1000
}

export interface Settings {
  version: number
  services: ServiceSettings
  ui: UISettings
}

const STORAGE_KEY = 'agentd:settings'
const CURRENT_VERSION = 1

export const DEFAULT_SETTINGS: Settings = {
  version: CURRENT_VERSION,
  services: {
    orchestratorUrl: import.meta.env.VITE_ORCHESTRATOR_URL ?? 'http://localhost:17006',
    notifyUrl: import.meta.env.VITE_NOTIFY_URL ?? 'http://localhost:17004',
    askUrl: import.meta.env.VITE_ASK_URL ?? 'http://localhost:17001',
  },
  ui: {
    theme: 'system',
    sidebarDefaultOpen: true,
    refreshInterval: 30,
    notificationsEnabled: true,
    logViewLines: 100,
  },
}

export function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return { ...DEFAULT_SETTINGS }
    const parsed = JSON.parse(raw) as Partial<Settings>
    // Deep-merge with defaults so newly added keys are always present
    return {
      ...DEFAULT_SETTINGS,
      ...parsed,
      services: {
        ...DEFAULT_SETTINGS.services,
        ...(parsed.services ?? {}),
      },
      ui: {
        ...DEFAULT_SETTINGS.ui,
        ...(parsed.ui ?? {}),
      },
    }
  } catch {
    return { ...DEFAULT_SETTINGS }
  }
}

export function saveSettings(s: Settings): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(s))
}

export function resetSettings(): Settings {
  const defaults = { ...DEFAULT_SETTINGS }
  saveSettings(defaults)
  return defaults
}
