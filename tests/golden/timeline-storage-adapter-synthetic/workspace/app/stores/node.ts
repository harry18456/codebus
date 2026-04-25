// Synthetic Pinia store — secondary Storage consumer (nice_to_have).
import { defineStore } from 'pinia'
import { useStorage } from '../composables/useStorage'
import type { EventNode } from '../types'

export const useNodeStore = defineStore('node', {
  state: () => ({ nodes: [] as EventNode[] }),
  actions: {
    async loadNode(id: string) {
      const { $storage } = useStorage()
      if (!$storage) return null
      return $storage.getNode(id)
    },
    async saveNode(node: EventNode) {
      const { $storage } = useStorage()
      if (!$storage) return
      await $storage.saveNode(node)
    },
  },
})
