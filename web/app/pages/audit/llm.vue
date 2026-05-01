<script setup lang="ts">
// /audit/llm — standalone page surfacing the LLM call inspector.
// Reads <ws>/.codebus/llm_calls.jsonl via Tauri IPC, lists rows by
// timestamp desc, opens the inspector overlay on row click.
//
// Spec: openspec/changes/llm-call-inspector-p0/specs/llm-call-inspector/spec.md
//   "/audit/llm page surfaces the inspector standalone"

import { computed, ref } from 'vue'
import { useRoute } from 'vue-router'

import LlmCallInspector from '~/components/audit/LlmCallInspector.vue'
import {
  useAuditJsonl,
  type LlmCallEntry
} from '~/composables/useAuditJsonl'

const route = useRoute()

const wsPath = computed<string | null>(() => {
  const raw = route.query.ws_path
  if (typeof raw !== 'string' || raw.length === 0) return null
  return raw
})

// Hooks must be called at top-level — defer the actual IPC inside the
// composable when wsPath is null by passing a sentinel that the
// composable rejects (we instead branch on wsPath at template level).
const audit = wsPath.value
  ? useAuditJsonl<LlmCallEntry>(wsPath.value, 'llm')
  : null

const activeRoles = ref<Set<string>>(new Set())
const activeModules = ref<Set<string>>(new Set())
const selectedUnderlyingIndex = ref<number | null>(null)
// `provider-settings-and-onboarding` Decision 7: PII detection rows
// hidden by default in both the inspector overlay and (future) panel.
const hidePiiDetection = ref(true)

const ROLE_OPTIONS = [
  'reasoning',
  'judge',
  'chat',
  'embed',
  'pii_detection'
] as const

const MODULE_OPTIONS = [
  'kb_build',
  'kb_query',
  'reasoning',
  'judge',
  'chat',
  'coverage',
  'generate',
  'qa_agent'
] as const

const filteredEntries = computed<LlmCallEntry[]>(() => {
  if (!audit) return []
  const all = audit.entries.value
  return all.filter((e) => {
    if (activeRoles.value.size > 0 && !activeRoles.value.has(e.role)) return false
    if (
      activeModules.value.size > 0 &&
      !activeModules.value.has(e.module ?? '')
    )
      return false
    return true
  })
})

// Display rows are filtered + reversed (newest first).
const displayRows = computed<LlmCallEntry[]>(() =>
  filteredEntries.value.slice().reverse()
)

function displayToUnderlying(displayIndex: number): number {
  return filteredEntries.value.length - 1 - displayIndex
}

function onRowClick(displayIndex: number): void {
  selectedUnderlyingIndex.value = displayToUnderlying(displayIndex)
}

function toggleRole(r: string): void {
  const next = new Set(activeRoles.value)
  next.has(r) ? next.delete(r) : next.add(r)
  activeRoles.value = next
  selectedUnderlyingIndex.value = null
}

function toggleModule(m: string): void {
  const next = new Set(activeModules.value)
  next.has(m) ? next.delete(m) : next.add(m)
  activeModules.value = next
  selectedUnderlyingIndex.value = null
}

function fmtCost(c: number | null | undefined): string {
  return typeof c === 'number' ? `$${c.toFixed(4)}` : '—'
}

const showTooLarge = computed(
  () => audit?.error.value?.message.includes('E_AUDIT_TOO_LARGE') ?? false
)
const showOtherError = computed(
  () =>
    audit?.error.value !== null &&
    audit?.error.value !== undefined &&
    !showTooLarge.value
)
</script>

<template>
  <div
    v-if="wsPath === null"
    data-testid="missing-ws-path"
    class="h-full grid place-items-center px-12"
  >
    <div
      class="max-w-[520px] p-6 rounded-lg bg-surface-1 border border-border-soft"
    >
      <h2 class="text-text-base font-semibold text-[16px] mb-2">
        缺少 ws_path
      </h2>
      <p class="text-text-dim text-[13.5px] leading-relaxed">
        本頁需要 <code class="font-mono">?ws_path=&lt;abs&gt;</code>
        query 參數指向 workspace 根目錄。
      </p>
    </div>
  </div>

  <div v-else-if="audit" class="grid grid-cols-[1fr_560px] h-full">
    <section class="overflow-y-auto bg-surface-0">
      <div class="px-6 py-5 max-w-[920px] mx-auto">
        <header class="mb-4">
          <h1 class="text-text-base text-[18px] font-semibold mb-1">
            LLM Calls audit
          </h1>
          <p class="font-mono text-[10.5px] text-text-mute">
            {{ wsPath }}/.codebus/llm_calls.jsonl
          </p>
        </header>

        <!-- Filter chips -->
        <div class="flex flex-wrap gap-2 mb-4 font-mono text-[11px]">
          <span class="text-text-mute">role:</span>
          <button
            v-for="r in ROLE_OPTIONS"
            :key="r"
            type="button"
            :data-chip="`role:${r}`"
            class="px-2 py-[2px] rounded border"
            :class="
              activeRoles.has(r)
                ? 'border-accent text-accent bg-accent/10'
                : 'border-border-base text-text-mute'
            "
            @click="toggleRole(r)"
          >
            {{ r }}
          </button>
          <span class="text-text-mute ml-2">module:</span>
          <button
            v-for="m in MODULE_OPTIONS"
            :key="m"
            type="button"
            :data-chip="`module:${m}`"
            class="px-2 py-[2px] rounded border"
            :class="
              activeModules.has(m)
                ? 'border-accent text-accent bg-accent/10'
                : 'border-border-base text-text-mute'
            "
            @click="toggleModule(m)"
          >
            {{ m }}
          </button>
        </div>

        <!-- States -->
        <div
          v-if="audit.loading.value"
          class="px-3 py-3 font-mono text-[11.5px] text-text-mute"
        >
          loading audit log…
        </div>
        <div
          v-else-if="showTooLarge"
          class="px-3 py-3 rounded border border-yellow/30 bg-yellow/10 text-[11.5px] text-text-dim"
        >
          audit too large for inline view (file exceeds 5 MiB cap; streaming
          UI lands in Phase 2)
        </div>
        <div
          v-else-if="showOtherError"
          class="px-3 py-3 rounded border border-red/30 bg-red/10 font-mono text-[11.5px] text-red"
        >
          {{ audit.error.value?.message }}
        </div>
        <div
          v-else-if="displayRows.length === 0"
          class="px-3 py-10 text-center text-text-mute text-[11.5px]"
        >
          no LLM calls in this workspace yet — run an Explorer or Q&amp;A task to
          populate.
        </div>
        <ul v-else class="divide-y divide-border-soft">
          <li
            v-for="(row, idx) in displayRows"
            :key="`${row.timestamp}-${row.request_id ?? idx}`"
            data-testid="llm-row"
            class="grid grid-cols-[110px_60px_80px_120px_70px_80px_1fr] gap-3 px-3 py-2 hover:bg-surface-2 cursor-pointer items-baseline font-mono text-[11px]"
            @click="onRowClick(idx)"
          >
            <span class="text-text-mute">{{ row.timestamp.split('T')[1]?.slice(0, 12) ?? row.timestamp }}</span>
            <span class="text-accent">{{ row.role }}</span>
            <span class="text-text-dim">{{ row.module ?? '—' }}</span>
            <span class="text-text-dim truncate">{{ row.model }}</span>
            <span class="text-text-base">{{ row.prompt_tokens + row.completion_tokens }}t</span>
            <span class="text-text-base">{{ fmtCost(row.cost_usd) }}</span>
            <span
              v-if="row.sanitizer_pass2_applied"
              class="text-purple text-right"
            >sanitize</span>
            <span v-else />
          </li>
        </ul>
      </div>
    </section>

    <LlmCallInspector
      :rows="filteredEntries"
      :active-index="selectedUnderlyingIndex"
      :hide-pii-detection="hidePiiDetection"
      @close="selectedUnderlyingIndex = null"
      @select-index="selectedUnderlyingIndex = $event"
      @toggle-pii-visible="hidePiiDetection = !hidePiiDetection"
    />
  </div>
</template>
