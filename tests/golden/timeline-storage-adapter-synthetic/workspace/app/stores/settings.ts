// Synthetic Pinia store — secondary Storage consumer (nice_to_have).
import { defineStore } from 'pinia'
import { useStorage } from '../composables/useStorage'
import type { AppSettings } from '../types'

export const useSettingsStore = defineStore('settings', {
  state: () => ({ settings: { locale: 'zh-TW' } as AppSettings }),
  actions: {
    async loadSettings() {
      const { $storage } = useStorage()
      if (!$storage) return
      this.settings = await $storage.getSettings()
    },
    async saveSettings(s: AppSettings) {
      const { $storage } = useStorage()
      if (!$storage) return
      await $storage.saveSettings(s)
      this.settings = s
    },
  },
})
