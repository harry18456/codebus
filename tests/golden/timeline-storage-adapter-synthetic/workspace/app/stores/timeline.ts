// Synthetic Pinia store — primary Storage consumer. Not valid TS.
// Mirrors `tests/golden/timeline-gdrive-adapter/ideal-route.md` 站 4 (consumer).
import { defineStore } from 'pinia'
import { useStorage } from '../composables/useStorage'
import type { TimelineConfig } from '../types'

export const useTimelineStore = defineStore('timeline', {
  state: () => ({
    timelines: [] as TimelineConfig[],
    loaded: false,
  }),
  actions: {
    async fetchTimelines() {
      const { $storage, $storageReady } = useStorage()
      if (!$storageReady || !$storage) return
      this.timelines = await $storage.listTimelines()
      this.loaded = true
    },
    async saveTimeline(cfg: TimelineConfig) {
      const { $storage } = useStorage()
      if (!$storage) return
      await $storage.saveTimeline(cfg)
    },
  },
})
