<script setup lang="ts">
// LlmCallInspector — drawer overlay surfacing one LLM call's full
// wire payload, response, tokens & cost, and timeline metadata.
// Pre-sanitize values are intentionally NOT rendered — `llm_calls.jsonl`
// only stores post-Pass-2 payloads (D-015 / D-022). The Wire payload
// tab annotates this with a banner when sanitizer_pass2_applied is true.
//
// Spec: openspec/changes/llm-call-inspector-p0/specs/llm-call-inspector/spec.md
//   "LlmCallInspector overlay renders four tabs and prev/next navigation"

import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { LlmCallEntry } from '~/composables/useAuditJsonl'

type TabKey = 'wire' | 'response' | 'tokens' | 'timeline'

const TAB_ORDER: TabKey[] = ['wire', 'response', 'tokens', 'timeline']
const TAB_LABELS: Record<TabKey, string> = {
  wire: 'Wire payload',
  response: 'Response',
  tokens: 'Tokens & cost',
  timeline: 'Timeline'
}
const EM_DASH = '—'

const props = defineProps<{
  rows: LlmCallEntry[]
  activeIndex: number | null
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'select-index', index: number): void
}>()

const activeTab = ref<TabKey>('wire')

watch(
  () => props.activeIndex,
  () => {
    activeTab.value = 'wire'
  }
)

const currentEntry = computed<LlmCallEntry | null>(() => {
  if (props.activeIndex === null) return null
  return props.rows[props.activeIndex] ?? null
})

const positionLabel = computed(() => {
  if (props.activeIndex === null) return ''
  return `${props.activeIndex + 1} / ${props.rows.length}`
})

function selectPrev(): void {
  if (props.activeIndex === null) return
  const next = Math.max(0, props.activeIndex - 1)
  emit('select-index', next)
}

function selectNext(): void {
  if (props.activeIndex === null) return
  const next = Math.min(props.rows.length - 1, props.activeIndex + 1)
  emit('select-index', next)
}

function handleKeyDown(e: KeyboardEvent): void {
  if (props.activeIndex === null) return
  if (e.key === 'Escape') {
    emit('close')
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeyDown)
})
onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeyDown)
})

function pretty(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

function tokenCellValue(n: number | null | undefined): string {
  return typeof n === 'number' ? n.toLocaleString() : EM_DASH
}

function costCellValue(c: number | null | undefined): string {
  if (typeof c !== 'number') return EM_DASH
  return `$${c.toFixed(4)}`
}

function latencyCellValue(ms: number | null | undefined): string {
  return typeof ms === 'number' ? `${ms} ms` : EM_DASH
}

function totalTokens(entry: LlmCallEntry): number {
  return (entry.prompt_tokens ?? 0) + (entry.completion_tokens ?? 0)
}
</script>

<template>
  <aside
    v-if="currentEntry !== null"
    class="fixed right-0 top-0 bottom-0 w-[560px] bg-surface-1 border-l border-border-soft shadow-2xl z-50 flex flex-col"
  >
    <!-- Header -->
    <header
      class="flex items-center gap-3 px-4 py-3 bg-surface-2 border-b border-border-soft"
    >
      <div class="flex-1 min-w-0">
        <div class="text-text-base font-semibold text-[13.5px]">
          LLM Call Inspector
        </div>
        <div
          class="font-mono text-[10.5px] text-text-mute mt-0.5 flex items-center gap-2"
        >
          <span class="text-text-dim">{{ currentEntry.request_id ?? EM_DASH }}</span>
          <span>·</span>
          <span class="text-text-dim">{{ currentEntry.timestamp }}</span>
        </div>
      </div>
      <div class="flex items-center gap-1.5 font-mono text-[11px] text-text-dim">
        <button
          type="button"
          data-action="prev"
          class="px-2 py-1 rounded hover:bg-surface-3 hover:text-text-base"
          aria-label="prev"
          @click="selectPrev"
        >
          ‹
        </button>
        <span class="px-1.5 text-text-base">{{ positionLabel }}</span>
        <button
          type="button"
          data-action="next"
          class="px-2 py-1 rounded hover:bg-surface-3 hover:text-text-base"
          aria-label="next"
          @click="selectNext"
        >
          ›
        </button>
      </div>
      <button
        type="button"
        data-action="close"
        class="ml-1 px-2 py-1 rounded hover:bg-surface-3 hover:text-text-base text-text-mute"
        aria-label="close"
        @click="emit('close')"
      >
        ✕
      </button>
    </header>

    <!-- Status strip -->
    <div
      class="flex flex-wrap items-center gap-2 px-4 py-2 border-b border-border-soft text-[10.5px] font-mono"
    >
      <span class="text-text-mute">role</span>
      <span
        class="px-2 py-[2px] rounded border border-accent text-accent"
      >
        {{ currentEntry.role }}
      </span>
      <span class="text-text-mute">module</span>
      <span class="px-2 py-[2px] rounded border border-border-base text-text-dim">
        {{ currentEntry.module ?? EM_DASH }}
      </span>
      <span class="text-text-mute">model</span>
      <span class="px-2 py-[2px] rounded border border-border-base text-text-dim">
        {{ currentEntry.model }}
      </span>
      <span
        v-if="currentEntry.sanitizer_pass2_applied"
        class="px-2 py-[2px] rounded border border-purple text-purple"
      >
        Pass 2 sanitize ON
      </span>
      <span class="ml-auto text-text-mute">
        latency
        <span class="text-text-base">{{ latencyCellValue(currentEntry.latency_ms) }}</span>
        · cost
        <span class="text-text-base">{{ costCellValue(currentEntry.cost_usd) }}</span>
      </span>
    </div>

    <!-- Tab switcher -->
    <div class="flex bg-surface-2 border-b border-border-soft">
      <button
        v-for="tab in TAB_ORDER"
        :key="tab"
        type="button"
        :data-tab="tab"
        class="px-3 py-2 font-mono text-[11px] border-b-2"
        :class="
          activeTab === tab
            ? 'text-text-base border-accent'
            : 'text-text-mute border-transparent hover:text-text-dim'
        "
        @click="activeTab = tab"
      >
        {{ TAB_LABELS[tab] }}
      </button>
    </div>

    <!-- Tab body -->
    <div class="flex-1 overflow-y-auto px-4 py-3">
      <!-- Wire payload -->
      <section v-if="activeTab === 'wire'">
        <div
          v-if="currentEntry.sanitizer_pass2_applied"
          data-testid="sanitize-banner"
          class="mb-3 px-3 py-2 rounded border border-purple/40 bg-purple/10 text-[11.5px] text-text-dim leading-relaxed"
        >
          <strong class="text-purple">Pass 2 sanitize ON</strong>
          —
          pre-sanitize values are not stored (D-015). Only the post-sanitize
          payload below was sent on the wire and recorded in
          <code class="font-mono">llm_calls.jsonl</code>.
        </div>
        <pre
          class="font-mono text-[11.5px] text-text-base bg-surface-2 rounded p-3 overflow-x-auto whitespace-pre-wrap"
        >{{ pretty(currentEntry.request) }}</pre>
      </section>

      <!-- Response -->
      <section v-else-if="activeTab === 'response'">
        <template v-if="currentEntry.response !== null">
          <pre
            class="font-mono text-[11.5px] text-text-base bg-surface-2 rounded p-3 overflow-x-auto whitespace-pre-wrap"
          >{{ pretty(currentEntry.response) }}</pre>
        </template>
        <div
          v-else
          class="px-3 py-3 rounded border border-border-soft bg-surface-2 text-[11.5px] text-text-dim leading-relaxed"
        >
          (no response — call may have failed; see error field if present)
          <div
            v-if="currentEntry.error"
            class="mt-2 font-mono text-[11px] text-red"
          >
            {{ currentEntry.error.class }}: {{ currentEntry.error.message }}
          </div>
        </div>
      </section>

      <!-- Tokens & cost -->
      <section v-else-if="activeTab === 'tokens'">
        <dl
          class="grid grid-cols-[140px_1fr] gap-x-4 gap-y-2 font-mono text-[12px]"
        >
          <dt class="text-text-mute">prompt_tokens</dt>
          <dd class="text-text-base">{{ tokenCellValue(currentEntry.prompt_tokens) }}</dd>
          <dt class="text-text-mute">completion_tokens</dt>
          <dd class="text-text-base">{{ tokenCellValue(currentEntry.completion_tokens) }}</dd>
          <dt class="text-text-mute">total</dt>
          <dd class="text-text-base">{{ tokenCellValue(totalTokens(currentEntry)) }}</dd>
          <dt class="text-text-mute">cost_usd</dt>
          <dd data-testid="cost-cell" class="text-text-base">
            {{ costCellValue(currentEntry.cost_usd) }}
          </dd>
          <dt class="text-text-mute">latency_ms</dt>
          <dd class="text-text-base">{{ latencyCellValue(currentEntry.latency_ms) }}</dd>
        </dl>
      </section>

      <!-- Timeline -->
      <section v-else>
        <dl
          class="grid grid-cols-[120px_1fr] gap-x-4 gap-y-2 font-mono text-[12px]"
        >
          <dt class="text-text-mute">module</dt>
          <dd class="text-text-base">{{ currentEntry.module ?? EM_DASH }}</dd>
          <dt class="text-text-mute">role</dt>
          <dd class="text-text-base">{{ currentEntry.role }}</dd>
          <dt class="text-text-mute">provider</dt>
          <dd class="text-text-base">{{ currentEntry.provider_id }}</dd>
          <dt class="text-text-mute">call_type</dt>
          <dd class="text-text-base">{{ currentEntry.call_type ?? EM_DASH }}</dd>
        </dl>
      </section>
    </div>
  </aside>
</template>
