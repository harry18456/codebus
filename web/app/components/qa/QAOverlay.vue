<script setup lang="ts">
// QAOverlay — drawer overlay component subscribing to the singleton
// useQaSession state. Pure presentation: rendering, dim-layer click, send
// composer; the layout host owns the keyboard listener.
//
// Spec: openspec/changes/qa-overlay-p0/specs/qa-overlay/spec.md
//   "<QAOverlay> drawer renders Q&A turns and listens for keyboard shortcuts"
//
// Design Decision (Drawer width 固定 480px、不可拖曳): Tailwind w-[480px],
// no resize handler. Half-transparent dim layer click → close; aside click
// stops propagation so it does not close.

import { computed, ref } from 'vue'
import { useQaSession } from '~/composables/useQaSession'
import QaTurnCard from './QaTurnCard.vue'

const session = useQaSession()
const composerInput = ref('')

const lastTurn = computed(() => {
  const list = session.turns.value
  return list.length > 0 ? list[list.length - 1] : null
})

const sendDisabled = computed(() => {
  if (!composerInput.value.trim()) return true
  if (lastTurn.value == null) return false
  return (
    lastTurn.value.status === 'pending' || lastTurn.value.status === 'streaming'
  )
})

const sessionBadge = computed(() => session.currentTaskId.value ?? '—')

const originStationId = computed(() => {
  const turn = lastTurn.value
  return turn?.originatingStationId ?? null
})

const addToKbCount = computed(() => {
  let count = 0
  for (const turn of session.turns.value) count += turn.kbGrowth.length
  return count
})

async function send(): Promise<void> {
  if (sendDisabled.value) return
  const prompt = composerInput.value.trim()
  composerInput.value = ''
  await session.start(prompt, originStationId.value)
}

function onComposerKeydown(e: KeyboardEvent): void {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    void send()
  }
}

function onAsideClick(e: Event): void {
  e.stopPropagation()
}

function onDimClick(): void {
  session.close()
}

function onNavigateToStation(stationId: string): void {
  // Drawer surface emits station-chip events to its host. P0: caller decides
  // whether to vue-router push or close the drawer first; QAOverlay itself
  // does not navigate, preserving the "navigation by page, not drawer"
  // convention from R-01.
  emit('navigate-to-station', stationId)
}

const emit = defineEmits<{
  (e: 'navigate-to-station', stationId: string): void
}>()
</script>

<template>
  <template v-if="session.open.value">
    <div
      data-testid="qa-dim-layer"
      class="fixed inset-0 z-40 bg-surface-0/60 backdrop-blur-sm"
      @click="onDimClick"
    />
    <aside
      class="fixed right-0 top-0 bottom-0 w-[480px] z-50 flex flex-col bg-surface-1 border-l border-border-base shadow-2xl"
      data-component="QAOverlay"
      @click="onAsideClick"
    >
      <!-- Header -->
      <header class="flex items-center gap-3 px-4 py-3 border-b border-border-soft bg-surface-2">
        <div class="flex-1 min-w-0">
          <div class="text-text-base font-semibold text-[13.5px]">
            Q&amp;A · Module 8
          </div>
          <div class="font-mono text-[10.5px] text-text-mute mt-0.5">
            session {{ sessionBadge }}
          </div>
        </div>
        <span
          v-if="originStationId"
          class="px-2 py-[2px] rounded border border-accent text-accent font-mono text-[10.5px]"
        >
          📍 {{ originStationId }}
        </span>
        <button
          type="button"
          aria-label="close"
          class="ml-1 px-2 py-1 rounded hover:bg-surface-3 hover:text-text-base text-text-mute"
          @click="session.close()"
        >
          ✕
        </button>
      </header>

      <!-- Body -->
      <div class="flex-1 overflow-y-auto px-4 py-3">
        <div
          v-if="session.turns.value.length === 0"
          class="text-text-mute text-[12px] leading-relaxed py-12 text-center"
        >
          Cmd+K 開始問問題
        </div>
        <div v-else class="flex flex-col gap-3">
          <QaTurnCard
            v-for="turn in session.turns.value"
            :key="turn.id"
            :turn="turn"
            @navigate-to-station="onNavigateToStation"
          />
        </div>
      </div>

      <!-- Composer -->
      <footer class="border-t border-border-soft bg-surface-2 px-4 py-3 flex flex-col gap-2">
        <div class="flex gap-2">
          <input
            v-model="composerInput"
            type="text"
            placeholder="ask a follow-up about this code…"
            class="flex-1 px-3 py-2 rounded border border-border-soft bg-surface-1 text-text-base text-[13px] placeholder:text-text-mute focus:outline-none focus:border-accent"
            @keydown="onComposerKeydown"
          >
          <button
            type="button"
            data-testid="qa-send-button"
            :disabled="sendDisabled"
            class="px-4 py-2 rounded text-[12.5px] font-medium border"
            :class="
              sendDisabled
                ? 'border-border-soft text-text-mute cursor-not-allowed'
                : 'border-accent text-accent hover:bg-accent/10'
            "
            @click="send"
          >
            送出
          </button>
        </div>
        <div class="flex items-center gap-3 font-mono text-[10.5px] text-text-mute">
          <span class="text-purple">Pass 3 sanitize on</span>
          <span>session add {{ addToKbCount }} / 20</span>
          <span>budget · 10 步</span>
        </div>
      </footer>
    </aside>
  </template>
</template>
