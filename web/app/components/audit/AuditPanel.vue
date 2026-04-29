<script setup lang="ts">
// AuditPanel surfaces seven workspace-level audit JSONL tabs declared by
// CLAUDE.md `七層 Audit JSONL`. The seven tab keys are non-negotiable; passing
// an unrecognised key fails at TypeScript compile time. Mockup sample data
// from `design/v1/shell.js` is fixture-only and MUST NOT appear in this
// component or anywhere else under `web/app/`.

import { computed } from 'vue'

const TAB_ORDER = [
  'sanitize',
  'tool',
  'reasoning',
  'token',
  'llm',
  'kb_growth',
  'generator'
] as const

export type AuditTab = (typeof TAB_ORDER)[number]

const TAB_LABELS: Record<AuditTab, string> = {
  sanitize: 'sanitize',
  tool: 'tool',
  reasoning: 'reason',
  token: 'token',
  llm: 'llm',
  kb_growth: 'kb_growth',
  generator: 'generator'
}

export type AuditBadgeKind = 'green' | 'yellow' | 'purple' | 'accent' | 'red'

export interface AuditRow {
  ts: string
  body: string
  badge?: string
  badgeKind?: AuditBadgeKind
}

interface Props {
  activeTab: AuditTab
  counts?: Partial<Record<AuditTab, number>>
  rows?: AuditRow[]
}

const props = withDefaults(defineProps<Props>(), {
  counts: () => ({}),
  rows: () => []
})

defineEmits<{
  (e: 'select-tab', tab: AuditTab): void
}>()

function badgeClass(kind?: AuditBadgeKind): string {
  switch (kind) {
    case 'green':
      return 'text-green border-green'
    case 'yellow':
      return 'text-yellow border-yellow'
    case 'purple':
      return 'text-purple border-purple'
    case 'accent':
      return 'text-accent border-accent'
    case 'red':
      return 'text-red border-red'
    default:
      return 'text-text-mute border-border-soft'
  }
}

const emptyMessage = computed(() => {
  const label = TAB_LABELS[props.activeTab]
  return `No ${label} events yet — they appear here as the sidecar streams them.`
})
</script>

<template>
  <div class="flex flex-col bg-surface-1 min-h-0 h-full">
    <div class="px-3.5 pt-2.5 border-b border-border-soft">
      <div
        class="font-mono text-[10.5px] tracking-[0.16em] text-text-mute uppercase flex items-center justify-between mb-2"
      >
        <span>workspace audit · &lt;ws&gt;/.codebus/</span>
        <span class="flex items-center gap-1.5 text-green">
          <span class="w-[5px] h-[5px] rounded-full bg-green" />
          LIVE
        </span>
      </div>
      <div class="flex flex-wrap gap-px bg-border-soft -mx-3.5 px-3.5">
        <button
          v-for="tab in TAB_ORDER"
          :key="tab"
          type="button"
          :data-tab="tab"
          class="px-2 py-1.5 font-mono text-[10.5px] flex items-center gap-1.5 flex-1 whitespace-nowrap min-w-0 bg-surface-1 border-b-2"
          :class="
            tab === activeTab
              ? 'text-text-base border-accent'
              : 'text-text-mute border-transparent hover:text-text-dim'
          "
          @click="$emit('select-tab', tab)"
        >
          {{ TAB_LABELS[tab] }}
          <span
            class="text-[9.5px] px-1 rounded-sm min-w-[18px] text-center"
            :class="
              tab === activeTab
                ? 'bg-surface-3 text-accent'
                : 'bg-surface-3 text-text-dim'
            "
          >
            {{ counts[tab] ?? 0 }}
          </span>
        </button>
      </div>
    </div>

    <div class="flex-1 overflow-y-auto py-1 min-h-0 font-mono text-[11px]">
      <div
        v-if="rows.length === 0"
        data-empty="true"
        class="px-4 py-10 text-center text-text-mute text-[11.5px] leading-relaxed"
      >
        {{ emptyMessage }}
      </div>
      <div
        v-for="(row, idx) in rows"
        v-else
        :key="`${row.ts}-${idx}`"
        class="px-3.5 py-2 border-b border-border-soft grid grid-cols-[56px_1fr_auto] gap-2.5 items-baseline hover:bg-surface-2 cursor-pointer"
      >
        <div class="text-text-mute text-[10px] pt-px">{{ row.ts }}</div>
        <div class="text-text-dim min-w-0 leading-[1.5]">{{ row.body }}</div>
        <div
          v-if="row.badge"
          class="font-mono text-[9px] px-1.5 py-px rounded-sm tracking-[0.04em] uppercase border self-start"
          :class="badgeClass(row.badgeKind)"
        >
          {{ row.badge }}
        </div>
      </div>
    </div>
  </div>
</template>
