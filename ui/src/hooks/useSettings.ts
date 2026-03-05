import { useState, useCallback } from 'react'
import { loadSettings, saveSettings, resetSettings } from '@/stores/settingsStore'
import type { Settings } from '@/stores/settingsStore'

export type { Settings }

export function useSettings() {
  const [settings, setSettings] = useState<Settings>(loadSettings)

  const update = useCallback((patch: Partial<Settings>) => {
    setSettings(prev => {
      const next = { ...prev, ...patch }
      saveSettings(next)
      return next
    })
  }, [])

  const updateServices = useCallback((patch: Partial<Settings['services']>) => {
    setSettings(prev => {
      const next = { ...prev, services: { ...prev.services, ...patch } }
      saveSettings(next)
      return next
    })
  }, [])

  const updateUI = useCallback((patch: Partial<Settings['ui']>) => {
    setSettings(prev => {
      const next = { ...prev, ui: { ...prev.ui, ...patch } }
      saveSettings(next)
      return next
    })
  }, [])

  const reset = useCallback(() => {
    const defaults = resetSettings()
    setSettings(defaults)
  }, [])

  return { settings, update, updateServices, updateUI, reset }
}
