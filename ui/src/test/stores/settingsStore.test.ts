import { describe, it, expect, beforeEach } from 'vitest'
import { loadSettings, saveSettings, resetSettings, DEFAULT_SETTINGS } from '@/stores/settingsStore'

const STORAGE_KEY = 'agentd:settings'

describe('settingsStore', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('loadSettings returns defaults when localStorage is empty', () => {
    const settings = loadSettings()
    expect(settings).toEqual(DEFAULT_SETTINGS)
  })

  it('saveSettings persists to localStorage', () => {
    const custom = {
      ...DEFAULT_SETTINGS,
      services: {
        ...DEFAULT_SETTINGS.services,
        orchestratorUrl: 'http://example.com:17006',
      },
    }
    saveSettings(custom)
    const raw = localStorage.getItem(STORAGE_KEY)
    expect(raw).not.toBeNull()
    const parsed = JSON.parse(raw!) as typeof custom
    expect(parsed.services.orchestratorUrl).toBe('http://example.com:17006')
  })

  it('loadSettings merges saved values with defaults', () => {
    // Save only a partial shape
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        version: 1,
        services: { orchestratorUrl: 'http://custom:9000' },
      }),
    )
    const settings = loadSettings()
    expect(settings.services.orchestratorUrl).toBe('http://custom:9000')
    // notifyUrl should fall back to default
    expect(settings.services.notifyUrl).toBe(DEFAULT_SETTINGS.services.notifyUrl)
    // ui should be fully default
    expect(settings.ui).toEqual(DEFAULT_SETTINGS.ui)
  })

  it('resetSettings returns defaults and clears localStorage entry', () => {
    saveSettings({
      ...DEFAULT_SETTINGS,
      services: {
        ...DEFAULT_SETTINGS.services,
        orchestratorUrl: 'http://custom:9000',
      },
    })

    const result = resetSettings()
    expect(result).toEqual(DEFAULT_SETTINGS)

    // After reset, loading should give defaults
    const reloaded = loadSettings()
    expect(reloaded.services.orchestratorUrl).toBe(DEFAULT_SETTINGS.services.orchestratorUrl)
  })

  it('loadSettings returns defaults when localStorage contains malformed JSON', () => {
    localStorage.setItem(STORAGE_KEY, 'not-valid-json{{{')
    const settings = loadSettings()
    expect(settings).toEqual(DEFAULT_SETTINGS)
  })
})
