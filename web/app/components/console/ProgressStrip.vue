<script setup lang="ts">
import { computed } from 'vue'
import type { ProgressSnapshot } from '~/composables/useExplorerStream'

const props = defineProps<{ progress: ProgressSnapshot | null }>()

type CellState = 'done' | 'now' | 'queued'

const indicator = computed<string>(() => {
  if (!props.progress) return 'step — / —'
  return `step ${props.progress.current} / ${props.progress.total}`
})

const cells = computed<CellState[]>(() => {
  if (!props.progress) return []
  const { current, total } = props.progress
  const safeTotal = Math.max(0, Math.floor(total))
  const nowIdx = current - 1
  return Array.from({ length: safeTotal }, (_, i) => {
    if (i < nowIdx) return 'done'
    if (i === nowIdx) return 'now'
    return 'queued'
  })
})
</script>

<template>
  <div
    class="flex items-center gap-3 px-4 py-2 border-b border-border-soft bg-surface-1"
  >
    <span class="font-mono text-[11px] text-text-dim whitespace-nowrap">
      {{ indicator }}
    </span>
    <div class="flex-1 grid gap-1.5" :style="{ gridTemplateColumns: `repeat(${Math.max(cells.length, 1)}, minmax(0, 1fr))` }">
      <div
        v-for="(state, idx) in cells"
        :key="idx"
        :data-state="state"
        class="h-1.5 rounded-sm"
        :class="{
          'bg-green': state === 'done',
          'bg-accent animate-pulse': state === 'now',
          'bg-surface-3': state === 'queued'
        }"
      />
    </div>
  </div>
</template>
