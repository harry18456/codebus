<script setup lang="ts">
// /audit/sanitizer — standalone page surfacing the Sanitizer Audit Inspector
// outside R-01 workspace chrome. Reads <ws>/.codebus/sanitize_audit.jsonl
// via Tauri IPC, lists rows by ts desc, opens the inspector overlay on
// row click.
//
// Spec: openspec/changes/sanitizer-audit-inspector-p0/specs/sanitizer-audit-inspector/spec.md
//   "/audit/sanitizer standalone page surfaces inspector outside R-01 workspace"

import { computed, ref } from 'vue'
import { useRoute } from 'vue-router'

import SanitizerAuditInspector from '~/components/audit/SanitizerAuditInspector.vue'
import { SANITIZER_AUDIT_BANNER } from '~/components/audit/sanitizerAuditBanner'
import {
  useSanitizeAudit,
  type SanitizeAuditEntry
} from '~/composables/useSanitizeAudit'
import { useSanitizerRules } from '~/composables/useSanitizerRules'

const route = useRoute()

const workspace = computed<string | null>(() => {
  const raw = route.query.workspace
  if (typeof raw !== 'string' || raw.length === 0) return null
  return raw
})

// Hooks must be called at top-level. When workspace is missing the page
// short-circuits to the empty-state branch without invoking Tauri IPC.
const audit = workspace.value ? useSanitizeAudit(workspace.value) : null

// Rules registry is fetched once per session (cached at module level).
// Lazy-load even when workspace is null so the rules drawer can warm
// regardless of which page mounts first.
const rules = useSanitizerRules()
void rules.loadOnce()

const selectedIndex = ref<number | null>(null)

const displayRows = computed<SanitizeAuditEntry[]>(() => {
  if (!audit) return []
  // Display newest-first by sorting on `ts` descending.
  return audit.entries.value.slice().sort((a, b) => b.ts.localeCompare(a.ts))
})

function displayToUnderlying(displayIndex: number): number {
  if (!audit) return -1
  const row = displayRows.value[displayIndex]
  if (!row) return -1
  return audit.entries.value.findIndex(
    (e) => e === row || (e.ts === row.ts && e.placeholder_index === row.placeholder_index && e.rule_id === row.rule_id)
  )
}

function onRowClick(displayIndex: number): void {
  selectedIndex.value = displayToUnderlying(displayIndex)
}

const currentRow = computed<SanitizeAuditEntry | null>(() => {
  if (!audit || selectedIndex.value === null) return null
  return audit.entries.value[selectedIndex.value] ?? null
})

// Renamed away from `showError` to avoid collision with Nuxt's auto-imported
// `showError` composable (nuxt/dist/app/composables/error). Vue template type
// inference resolves bare `showError` to the always-defined auto-import,
// which yields TS2774 ("condition will always return true since this
// function is always defined"). See change fix-phase7-typecheck-baseline.
const hasError = computed(() => audit?.error.value !== null && audit?.error.value !== undefined)
</script>

<template>
  <div
    v-if="workspace === null"
    data-testid="missing-workspace"
    class="h-full grid place-items-center px-12"
  >
    <div
      class="max-w-[520px] p-6 rounded-lg bg-surface-1 border border-border-soft"
    >
      <div
        class="mb-4 px-3 py-2 rounded border border-purple/40 bg-purple/12 text-[11.5px] text-text-base whitespace-pre-line"
      >{{ SANITIZER_AUDIT_BANNER }}</div>
      <h2 class="text-text-base font-semibold text-[16px] mb-2">
        缺少 workspace
      </h2>
      <p class="text-text-dim text-[13.5px] leading-relaxed">
        本頁需要 <code class="font-mono">?workspace=&lt;abs&gt;</code>
        query 參數指向 workspace 根目錄。
      </p>
    </div>
  </div>

  <div v-else class="grid grid-cols-[1fr_560px] h-full">
    <section class="overflow-y-auto bg-surface-0">
      <div class="px-6 py-5 max-w-[920px] mx-auto">
        <div
          class="mb-4 px-3 py-2 rounded border border-purple/40 bg-purple/12 text-[11.5px] text-text-base whitespace-pre-line"
        >{{ SANITIZER_AUDIT_BANNER }}</div>

        <header class="mb-4">
          <h1 class="text-text-base text-[18px] font-semibold mb-1">
            Sanitizer Audit
          </h1>
          <p class="font-mono text-[10.5px] text-text-mute">
            {{ workspace }}/.codebus/sanitize_audit.jsonl
          </p>
        </header>

        <div
          v-if="audit?.loading.value"
          class="px-3 py-3 font-mono text-[11.5px] text-text-mute"
        >
          loading audit log…
        </div>
        <div
          v-else-if="hasError"
          class="px-3 py-3 rounded border border-red/30 bg-red/10 font-mono text-[11.5px] text-red"
        >
          {{ audit?.error.value?.message }}
        </div>
        <div
          v-else-if="displayRows.length === 0"
          class="px-3 py-10 text-center text-text-mute text-[11.5px]"
        >
          no sanitize events in this workspace yet.
        </div>
        <ul v-else class="divide-y divide-border-soft">
          <li
            v-for="(row, idx) in displayRows"
            :key="`${row.ts}-${idx}`"
            data-testid="sanitize-row"
            class="grid grid-cols-[140px_70px_1fr_auto] gap-3 px-3 py-2 hover:bg-surface-2 cursor-pointer items-baseline font-mono text-[11px]"
            @click="onRowClick(idx)"
          >
            <span class="text-text-mute">{{ row.ts.split('T')[1]?.slice(0, 12) ?? row.ts }}</span>
            <span
              class="px-1.5 py-px rounded-sm border border-purple/40 text-purple text-[10px]"
            >Pass {{ row.pass }}</span>
            <span
              class="px-1.5 py-px rounded-sm border border-purple/40 bg-purple/12 text-purple text-[10px]"
            >&lt;REDACTED:{{ row.kind }}#{{ row.placeholder_index }}&gt;</span>
            <span class="text-text-dim">{{ row.rule_id }}</span>
          </li>
        </ul>
      </div>
    </section>

    <SanitizerAuditInspector
      v-if="currentRow !== null"
      :row="currentRow"
      @close="selectedIndex = null"
    />
  </div>
</template>
