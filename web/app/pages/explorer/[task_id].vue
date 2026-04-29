<script setup lang="ts">
// Explorer console page — live SSE timeline + audit panel for one Module 4
// Explorer task. The page owns exactly one useExplorerStream instance per
// task_id; navigating between two /explorer/{task_id} routes closes the
// prior connection BEFORE opening the next so two streams never coexist.
//
// Spec: openspec/changes/agent-console-p0/specs/agent-console/spec.md
//   "Explorer console page mounts on /explorer/{task_id} route"
//   "AuditPanel reasoning tab consumes useExplorerStream auditRows"

import { computed, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import { useRoute } from 'vue-router'

import AuditPanel, {
  type AuditRow,
  type AuditTab
} from '~/components/audit/AuditPanel.vue'
import LlmCallInspector from '~/components/audit/LlmCallInspector.vue'
import ConsoleTimeline from '~/components/console/ConsoleTimeline.vue'
import CoverageBanner from '~/components/console/CoverageBanner.vue'
import ProgressStrip from '~/components/console/ProgressStrip.vue'
import {
  useAuditJsonl,
  type LlmCallEntry,
  type UseAuditJsonlApi
} from '~/composables/useAuditJsonl'
import {
  useExplorerStream,
  type UseExplorerStreamApi
} from '~/composables/useExplorerStream'

const TASK_ID_RE = /^explore_[0-9a-f]{8}$/

const route = useRoute()
const taskId = computed(() => String(route.params.task_id ?? ''))
const taskIdValid = computed(() => TASK_ID_RE.test(taskId.value))
const wsPath = computed<string | null>(() => {
  const raw = route.query.ws_path
  return typeof raw === 'string' && raw.length > 0 ? raw : null
})

// shallowRef so swapping the whole api object doesn't deep-track its inner
// reactive Map / refs (which are already independently reactive).
const stream = shallowRef<UseExplorerStreamApi | null>(null)
const llmAudit = shallowRef<UseAuditJsonlApi<LlmCallEntry> | null>(null)
const inspectorIndex = ref<number | null>(null)
const activeTab = ref<AuditTab>('reasoning')

const stepBuckets = computed(
  () => stream.value?.stepBuckets.value ?? new Map()
)
const progress = computed(() => stream.value?.progress.value ?? null)
const coverage = computed(() => stream.value?.coverageBanner.value ?? null)
const budget = computed(() => stream.value?.budgetBanner.value ?? {})
const auditRows = computed<AuditRow[]>(
  () => stream.value?.auditRows.value ?? []
)

// Per spec: each tab is bound to its dedicated audit source. Reasoning →
// useExplorerStream auditRows. LLM → useAuditJsonl(ws_path, 'llm') with
// live-tail from the explorer stream. Other tabs → empty (placeholder).
const llmRowsAsAuditRows = computed<AuditRow[]>(() => {
  const entries = llmAudit.value?.entries.value ?? []
  return entries
    .slice()
    .reverse()
    .map((e) => {
      const tsRaw = typeof e.timestamp === 'string' ? e.timestamp : ''
      const ts = tsRaw.includes('T')
        ? (tsRaw.split('T')[1]?.slice(0, 8) ?? tsRaw)
        : tsRaw || '—'
      // Live-tail events use { tokens: { prompt, completion } } shape
      // while disk entries use prompt_tokens / completion_tokens at top
      // level. Read both shapes defensively so the mapper stays single.
      const prompt =
        e.prompt_tokens ??
        (e as unknown as { tokens?: { prompt?: number } }).tokens?.prompt ??
        0
      const completion =
        e.completion_tokens ??
        (e as unknown as { tokens?: { completion?: number } }).tokens
          ?.completion ??
        0
      return {
        ts,
        body: `${e.role} · ${e.module ?? '—'} · ${e.model} · ${prompt + completion}t`,
        badge: e.sanitizer_pass2_applied ? 'sanitize' : undefined,
        badgeKind: e.sanitizer_pass2_applied
          ? ('purple' as const)
          : undefined
      }
    })
})

const tabRows = computed<AuditRow[]>(() => {
  if (activeTab.value === 'reasoning') return auditRows.value
  if (activeTab.value === 'llm') return llmRowsAsAuditRows.value
  return []
})
const tabCounts = computed(() => ({
  reasoning: auditRows.value.length,
  llm: llmAudit.value?.entries.value.length ?? 0
}))

const showLlmFallback = computed(
  () => activeTab.value === 'llm' && wsPath.value === null
)

watch(
  taskId,
  (newId, oldId) => {
    if (oldId !== undefined && oldId !== newId && stream.value) {
      // Close prior connection BEFORE constructing the next so two
      // EventSources never coexist for the same page render.
      stream.value.close()
      stream.value = null
      llmAudit.value = null
      inspectorIndex.value = null
    }
    if (taskIdValid.value) {
      const s = useExplorerStream(newId)
      stream.value = s
      // Wire up llm audit when ws_path is present. Live-tail piggybacks
      // on this single explorer stream — no second EventSource.
      if (wsPath.value !== null) {
        llmAudit.value = useAuditJsonl<LlmCallEntry>(wsPath.value, 'llm', {
          liveTailFromExplorerStream: s
        })
      }
    }
  },
  { immediate: true }
)

onBeforeUnmount(() => {
  stream.value?.close()
  stream.value = null
})

function selectTab(tab: AuditTab): void {
  activeTab.value = tab
  inspectorIndex.value = null
}

function onAuditRowSelect(displayIndex: number): void {
  if (activeTab.value !== 'llm' || !llmAudit.value) return
  // Display rows are reversed; translate back to underlying index.
  const total = llmAudit.value.entries.value.length
  inspectorIndex.value = total - 1 - displayIndex
}
</script>

<template>
  <div
    v-if="!taskIdValid"
    data-testid="invalid-task-id"
    class="h-full grid place-items-center px-12"
  >
    <div
      class="max-w-[520px] p-6 rounded-lg bg-surface-1 border border-border-soft"
    >
      <h2 class="text-text-base font-semibold text-[16px] mb-2">
        無效的 task_id
      </h2>
      <p class="text-text-dim text-[13.5px] leading-relaxed mb-4">
        URL 中的 task_id 必須符合
        <code class="font-mono text-text-base">explore_xxxxxxxx</code>
        （8 個 hex 字元）。
      </p>
      <p class="font-mono text-[12px] text-text-mute break-all">
        收到：{{ taskId }}
      </p>
    </div>
  </div>

  <div v-else class="grid grid-cols-[1fr_360px] h-full">
    <section class="overflow-y-auto bg-surface-0">
      <div class="px-9 py-6 max-w-[880px] mx-auto flex flex-col gap-5">
        <header
          class="flex items-center justify-between px-4 py-3 bg-surface-1 border border-border-soft rounded-[10px]"
        >
          <div>
            <div class="font-mono text-[10.5px] tracking-[0.14em] uppercase text-text-mute">
              task
            </div>
            <div class="font-mono text-[13px] text-text-base mt-0.5">
              {{ taskId }}
            </div>
          </div>
          <div class="font-mono text-[10.5px] text-text-dim flex items-center gap-2">
            <span
              class="w-1.5 h-1.5 rounded-full"
              :class="stream?.done.value ? 'bg-green' : 'bg-accent animate-pulse'"
            />
            {{ stream?.done.value ? 'done' : stream?.status.value ?? '—' }}
          </div>
        </header>

        <ProgressStrip :progress="progress" />
        <CoverageBanner :coverage="coverage" :budget="budget" />
        <ConsoleTimeline :step-buckets="stepBuckets" />
      </div>
    </section>

    <aside class="border-l border-border-soft min-h-0 relative">
      <AuditPanel
        :active-tab="activeTab"
        :rows="tabRows"
        :counts="tabCounts"
        @select-tab="selectTab"
        @select-row="onAuditRowSelect"
      />
      <div
        v-if="showLlmFallback"
        class="absolute inset-0 px-4 py-6 bg-surface-1 text-text-dim text-[12px] leading-relaxed pointer-events-none"
      >
        <div
          class="border border-yellow/30 bg-yellow/10 rounded p-3 font-mono text-[11px] text-text-base"
        >
          ws_path required for LLM audit binding
          <p class="mt-1 text-text-mute leading-relaxed">
            navigate via the R-01 / generator flow that supplies
            <code>?ws_path=&lt;abs&gt;</code>; the SSE stream is open but the
            <code>llm_calls.jsonl</code> reader needs the workspace path.
          </p>
        </div>
      </div>
    </aside>
    <LlmCallInspector
      :rows="llmAudit?.entries.value ?? []"
      :active-index="inspectorIndex"
      @close="inspectorIndex = null"
      @select-index="inspectorIndex = $event"
    />
  </div>
</template>
