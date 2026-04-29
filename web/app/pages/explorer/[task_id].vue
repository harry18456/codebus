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
import ConsoleTimeline from '~/components/console/ConsoleTimeline.vue'
import CoverageBanner from '~/components/console/CoverageBanner.vue'
import ProgressStrip from '~/components/console/ProgressStrip.vue'
import {
  useExplorerStream,
  type UseExplorerStreamApi
} from '~/composables/useExplorerStream'

const TASK_ID_RE = /^explore_[0-9a-f]{8}$/

const route = useRoute()
const taskId = computed(() => String(route.params.task_id ?? ''))
const taskIdValid = computed(() => TASK_ID_RE.test(taskId.value))

// shallowRef so swapping the whole api object doesn't deep-track its inner
// reactive Map / refs (which are already independently reactive).
const stream = shallowRef<UseExplorerStreamApi | null>(null)
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

// Per spec: only the reasoning tab consumes live auditRows in P0; other tabs
// receive an empty array so AuditPanel renders its empty-state placeholder.
const tabRows = computed<AuditRow[]>(() =>
  activeTab.value === 'reasoning' ? auditRows.value : []
)
const tabCounts = computed(() => ({ reasoning: auditRows.value.length }))

watch(
  taskId,
  (newId, oldId) => {
    if (oldId !== undefined && oldId !== newId && stream.value) {
      // Close prior connection BEFORE constructing the next so two
      // EventSources never coexist for the same page render.
      stream.value.close()
      stream.value = null
    }
    if (taskIdValid.value) {
      stream.value = useExplorerStream(newId)
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

    <aside class="border-l border-border-soft min-h-0">
      <AuditPanel
        :active-tab="activeTab"
        :rows="tabRows"
        :counts="tabCounts"
        @select-tab="selectTab"
      />
    </aside>
  </div>
</template>
