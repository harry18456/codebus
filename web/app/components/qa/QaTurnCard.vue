<script setup lang="ts">
// QaTurnCard — single Q&A turn rendered as four conditional phases.
//
// Spec: openspec/changes/qa-overlay-p0/specs/qa-overlay/spec.md
//   "<QaTurnCard> renders four phases per turn"
//
// Per design Decision (Multi-turn 視覺骨架): each turn carries its own
// `ragHits / reactSteps / kbGrowth / answer` slots and renders only the
// phases it has. Status badge maps `pending|streaming|done|error` to a
// minimal visual; rollback button is forbidden in P0 (Phase 2 territory).

import { computed } from 'vue'
import type { QaTurn } from '~/composables/useQaSession'
import QaCitations from './QaCitations.vue'

const props = defineProps<{
  turn: QaTurn
}>()

defineEmits<{
  (e: 'navigate-to-station', stationId: string): void
}>()

const statusBadge = computed<{ label: string; tone: string }>(() => {
  switch (props.turn.status) {
    case 'pending':
      return { label: '等候中…', tone: 'border-text-mute text-text-mute' }
    case 'streaming':
      return { label: '進行中…', tone: 'border-accent text-accent animate-pulse' }
    case 'done':
      return { label: '', tone: 'border-green text-green' }
    case 'error':
      return { label: '錯誤', tone: 'border-red text-red' }
    default:
      return { label: '', tone: 'border-text-mute text-text-mute' }
  }
})

function fmtScore(s: number): string {
  return s.toFixed(2)
}

function snippetCap(s: string): string {
  return s.length > 120 ? s.slice(0, 120) + '…' : s
}
</script>

<template>
  <article
    class="rounded-lg border border-border-soft bg-surface-1 p-4 flex flex-col gap-3"
    :data-turn-id="turn.id"
  >
    <!-- Status badge -->
    <div class="flex items-center justify-between">
      <div
        v-if="turn.originatingStationId"
        class="font-mono text-[10.5px] text-text-mute"
      >
        📍 {{ turn.originatingStationId }}
      </div>
      <span
        v-if="turn.status !== 'done'"
        :data-status="turn.status"
        class="px-2 py-[2px] rounded border text-[10.5px] font-mono ml-auto"
        :class="statusBadge.tone"
      >
        {{ statusBadge.label }}
      </span>
      <span
        v-else
        :data-status="turn.status"
        class="w-1.5 h-1.5 rounded-full bg-green ml-auto"
      />
    </div>

    <!-- Phase 1: User message -->
    <div class="text-text-base text-[13px]">
      <span class="text-text-mute font-mono text-[10.5px] mr-2">Q</span>
      {{ turn.question }}
    </div>

    <!-- Phase 2: RAG hits -->
    <section v-if="turn.ragHits !== null" class="flex flex-col gap-2">
      <header class="font-mono text-[10.5px] tracking-[0.14em] uppercase text-text-mute">
        ① RAG 探查
      </header>
      <ul class="flex flex-col gap-1.5">
        <li
          v-for="(hit, idx) in turn.ragHits"
          :key="`${hit.file_path}-${hit.line_start}-${idx}`"
          class="rounded border border-border-soft bg-surface-2 px-3 py-2 font-mono text-[11px]"
        >
          <div class="flex items-baseline gap-2">
            <span class="text-text-base">
              {{ hit.file_path }}:{{ hit.line_start }}-{{ hit.line_end }}
            </span>
            <span class="text-text-mute">score {{ fmtScore(hit.score) }}</span>
          </div>
          <div class="text-text-dim mt-1 whitespace-pre-wrap break-all">
            {{ snippetCap(hit.snippet) }}
          </div>
          <div class="flex flex-wrap gap-1 mt-1">
            <span
              v-for="stationId in hit.related_stations"
              :key="stationId"
              :data-station-id="stationId"
              class="px-1.5 py-px rounded border border-accent text-accent text-[10px] cursor-pointer"
              @click="$emit('navigate-to-station', stationId)"
            >
              📍 {{ stationId }}
            </span>
          </div>
        </li>
      </ul>
    </section>

    <!-- Phase 3: ReAct steps -->
    <section v-if="turn.reactSteps.length > 0" class="flex flex-col gap-2">
      <header class="font-mono text-[10.5px] tracking-[0.14em] uppercase text-text-mute">
        ② ReAct loop
      </header>
      <div
        v-for="step in turn.reactSteps"
        :key="step.step"
        class="rounded border border-border-soft bg-surface-2 px-3 py-2 font-mono text-[11px] flex flex-col gap-1.5"
      >
        <div class="text-text-mute">
          step {{ step.step }}
        </div>
        <div v-if="step.thought" class="text-text-dim whitespace-pre-wrap">
          {{ step.thought.text }}
        </div>
        <div
          v-for="(action, aIdx) in step.actions"
          :key="aIdx"
          class="text-text-base"
          :class="action.isError ? 'text-red' : ''"
        >
          <span class="text-text-mute">{{ action.tool }} →</span>
          <span class="ml-1 whitespace-pre-wrap">{{ action.observation }}</span>
        </div>
      </div>

      <!-- KB growth block (inline within ReAct section, P0 is read-only) -->
      <div
        v-if="turn.kbGrowth.length > 0"
        class="rounded border border-purple/40 bg-purple/12 px-3 py-2 flex flex-col gap-2"
        data-testid="qa-kb-growth-block"
      >
        <header class="font-mono text-[10.5px] tracking-[0.14em] uppercase text-purple">
          KB 沉澱
        </header>
        <div
          v-for="(growth, gIdx) in turn.kbGrowth"
          :key="growth.entry_id ?? gIdx"
          class="font-mono text-[11px] flex flex-col gap-1"
        >
          <div class="text-text-base">
            <span class="text-text-mute">entry</span>
            <span class="ml-1">{{ growth.entry_id }}</span>
          </div>
          <div class="text-text-dim">{{ growth.source }}</div>
          <div v-if="growth.reason" class="text-text-dim italic">
            {{ growth.reason }}
          </div>
          <div class="flex flex-wrap gap-1">
            <span
              v-for="stationId in growth.related_stations"
              :key="stationId"
              class="px-1.5 py-px rounded border border-accent text-accent text-[10px]"
            >
              📍 {{ stationId }}
            </span>
          </div>
        </div>
      </div>
    </section>

    <!-- Phase 4: Answer -->
    <section v-if="turn.answer !== null" class="flex flex-col gap-2">
      <header class="font-mono text-[10.5px] tracking-[0.14em] uppercase text-text-mute">
        ③ 回答
      </header>
      <div class="text-text-base text-[13px] leading-relaxed whitespace-pre-wrap">
        {{ turn.answer.text }}
      </div>
      <QaCitations
        :citations="turn.answer.citations"
        @navigate-to-station="$emit('navigate-to-station', $event)"
      />
    </section>

    <!-- Error surfacing -->
    <div
      v-if="turn.status === 'error' && turn.error"
      class="rounded border border-red/40 bg-red/10 px-3 py-2 font-mono text-[11px] text-red"
    >
      {{ turn.error.message }}
    </div>
  </article>
</template>
