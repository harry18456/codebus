<script setup lang="ts">
// `<OnrampProgress>` — pre-station SSE progress strip used by the
// entry-page workspace onramp. Renders a zh-TW phase label + the
// most recent progress counter from the active sidecar task plus a
// rolling elapsed-seconds timer.
//
// Backs SHALL clauses in
// openspec/changes/entry-workspace-onramp/specs/workspace-onramp/spec.md
//   Requirement: Entry page exposes folder-picker workspace onramp
//   Requirement: Workspace onramp drives scan, kb-build, explore, then generate via SSE
//
// Per design Decision 2 this is a NEW component (not a reuse of
// `<ProgressStrip>`) — ProgressStrip's bucket-fill UI is in-station;
// the onramp wants throughput numbers ("掃描中…42/120 檔案") instead.

import { computed, onUnmounted, ref, watch } from 'vue'
import type { SseEvent } from '~/composables/useSseTask'
import type { OnrampPhase } from '~/composables/useWorkspaceOnramp'

interface Props {
  phase: OnrampPhase
  events: SseEvent[]
}

const props = defineProps<Props>()

const PHASE_LABEL: Record<string, string> = {
  scanning: '掃描中',
  indexing: '建立索引中',
  exploring: '探索中',
  generating: '產生教學中'
}

const phaseLabel = computed<string>(() => PHASE_LABEL[props.phase] ?? '')

interface ProgressData {
  current?: number
  total?: number
  phase?: string
  current_file?: string
}
interface AgentThoughtData {
  step?: number
}

const latestProgress = computed<ProgressData | null>(() => {
  for (let i = props.events.length - 1; i >= 0; i -= 1) {
    const ev = props.events[i]
    if (ev?.type === 'progress') return (ev.data ?? {}) as ProgressData
  }
  return null
})

const latestStep = computed<number | null>(() => {
  for (let i = props.events.length - 1; i >= 0; i -= 1) {
    const ev = props.events[i]
    if (ev?.type === 'agent_thought') {
      const data = (ev.data ?? {}) as AgentThoughtData
      if (typeof data.step === 'number') return data.step
    }
  }
  return null
})

const counterText = computed<string>(() => {
  if (props.phase === 'exploring' && latestStep.value !== null) {
    return `step ${latestStep.value}`
  }
  const p = latestProgress.value
  if (p && typeof p.current === 'number') {
    if (typeof p.total === 'number' && p.total > 0) {
      return `${p.current} / ${p.total}`
    }
    return `${p.current}`
  }
  return ''
})

// Elapsed-seconds timer. Starts on first mount with an in-flight phase
// and resets whenever the phase transitions into a new in-flight
// phase. Lives in component scope (not the singleton composable) so a
// remount on a fresh phase always shows a fresh "0s".
const elapsedSec = ref<number>(0)
const startedAt = ref<number>(Date.now())
let intervalId: ReturnType<typeof setInterval> | null = null

function tick(): void {
  elapsedSec.value = Math.floor((Date.now() - startedAt.value) / 1000)
}

function startTicker(): void {
  startedAt.value = Date.now()
  elapsedSec.value = 0
  if (intervalId === null) {
    intervalId = setInterval(tick, 1000)
  }
}

function stopTicker(): void {
  if (intervalId !== null) {
    clearInterval(intervalId)
    intervalId = null
  }
}

const inFlightPhases: ReadonlyArray<OnrampPhase> = [
  'scanning',
  'indexing',
  'exploring',
  'generating'
]

watch(
  () => props.phase,
  (next, prev) => {
    const nextInFlight = inFlightPhases.includes(next as OnrampPhase)
    if (next !== prev) {
      if (nextInFlight) {
        startTicker()
      } else {
        stopTicker()
      }
    }
  },
  { immediate: true }
)

onUnmounted(stopTicker)
</script>

<template>
  <div
    data-testid="onramp-progress"
    class="flex flex-col gap-1 px-3 py-2 rounded-md bg-surface-1 border border-border-base"
  >
    <div class="flex items-center justify-between text-[12.5px] text-text-base">
      <span class="font-medium">{{ phaseLabel }}</span>
      <span class="text-text-mute">{{ elapsedSec }}s</span>
    </div>
    <div
      v-if="counterText"
      class="text-[12px] text-text-mute font-mono"
      data-testid="onramp-progress-counter"
    >
      {{ counterText }}
    </div>
    <div
      v-if="latestProgress?.current_file"
      class="text-[11.5px] text-text-mute truncate font-mono"
      data-testid="onramp-progress-file"
    >
      {{ latestProgress.current_file }}
    </div>
  </div>
</template>
