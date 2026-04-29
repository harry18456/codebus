<script lang="ts">
// Re-export the D-015 banner constant so callers can import it directly
// from the SFC module (`import { SANITIZER_AUDIT_BANNER } from '...vue'`)
// in addition to the sibling .ts file. Both routes hit the same string.
export { SANITIZER_AUDIT_BANNER } from './sanitizerAuditBanner'
</script>

<script setup lang="ts">
// SanitizerAuditInspector — overlay that surfaces the ten metadata
// fields recorded for one `sanitize_audit.jsonl` row. Strict P0 contract
// (D-015 invariant): metadata-only, no raw value reconstruction, no
// network call beyond the inspector module's allowed dependencies.
//
// Spec: openspec/changes/sanitizer-audit-inspector-p0/specs/sanitizer-audit-inspector/spec.md
//   "SanitizerAuditInspector overlay renders metadata-only view of a sanitize_audit row"
//   "SanitizerAuditInspector displays a D-015 banner verbatim"
//
// Forward-reference (P1+) — `sanitizer-audit-unlock` capability will own
// any path that re-exposes pre-sanitize values; this overlay deliberately
// has no such surface.

import { computed, onBeforeUnmount, onMounted } from 'vue'
import {
  PASS_LABELS,
  type SanitizeAuditEntry,
  type SanitizeSourceView
} from '~/composables/useSanitizeAudit'
import { useSanitizerRules } from '~/composables/useSanitizerRules'
import { SANITIZER_AUDIT_BANNER } from './sanitizerAuditBanner'

const props = defineProps<{
  row: SanitizeAuditEntry
}>()

const emit = defineEmits<{
  (e: 'close'): void
}>()

const rules = useSanitizerRules()
// Lazy-load the registry once. The composable cache ensures one fetch per
// session; the loadOnce promise here is fire-and-forget — rendering with
// `lookup() === null` is the documented fallback.
void rules.loadOnce()

const ruleEntry = computed(() => rules.lookup(props.row.rule_id))

const sourceView = computed<SanitizeSourceView>(() => parseSource(props.row.source))

const placeholderToken = computed<string>(
  () => `<REDACTED:${props.row.kind}#${props.row.placeholder_index}>`
)

const passLabel = computed<string>(() => {
  const fromMap = PASS_LABELS[props.row.pass as 1 | 2 | 3]
  return fromMap ?? `Pass ${props.row.pass}`
})

const isAllowlisted = computed<boolean>(
  () => props.row.extra?.allowlisted === true
)

const otherExtraEntries = computed<Array<[string, unknown]>>(() => {
  const extra = props.row.extra ?? {}
  return Object.entries(extra).filter(([k, v]) => !(k === 'allowlisted' && v === true))
})

const PASS_LABEL_PREFIX: Record<string, string> = {
  scanner: 'Scanner',
  provider: 'Provider',
  add_to_kb: 'Q&A add_to_kb'
}

function parseSource(raw: SanitizeAuditEntry['source']): SanitizeSourceView {
  if (typeof raw === 'string') {
    if (raw.startsWith('file:')) {
      const path = raw.slice('file:'.length)
      return { kind: 'file', pass: null, path, label: path }
    }
    if (raw.startsWith('message:')) {
      const messageId = raw.slice('message:'.length)
      return {
        kind: 'message',
        pass: null,
        message_id: messageId,
        label: `message ${messageId}`
      }
    }
    return { kind: 'unknown', pass: null, label: '(unknown source format)', raw }
  }
  if (raw && typeof raw === 'object') {
    const passValue = typeof raw.pass === 'string' ? raw.pass : null
    const pathValue = typeof raw.path === 'string' ? raw.path : null
    if (pathValue !== null) {
      const labelPrefix =
        passValue !== null
          ? PASS_LABEL_PREFIX[passValue] ?? passValue
          : null
      const label = labelPrefix !== null ? `${labelPrefix} · ${pathValue}` : pathValue
      return { kind: 'file', pass: passValue, path: pathValue, label }
    }
  }
  return { kind: 'unknown', pass: null, label: '(unknown source format)', raw }
}

function rawSourceJson(view: SanitizeSourceView): string {
  if (view.kind !== 'unknown') return ''
  try {
    return JSON.stringify(view.raw, null, 2)
  } catch {
    return String(view.raw)
  }
}

function handleKeyDown(e: KeyboardEvent): void {
  if (e.key === 'Escape') emit('close')
}

onMounted(() => {
  window.addEventListener('keydown', handleKeyDown)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeyDown)
})
</script>

<template>
  <aside
    class="fixed right-0 top-0 bottom-0 w-[560px] bg-surface-1 border-l border-border-soft shadow-2xl z-50 flex flex-col"
    data-component="SanitizerAuditInspector"
  >
    <!-- Sticky D-015 banner -->
    <div
      data-testid="sanitizer-banner"
      class="sticky top-0 z-10 px-4 py-2 bg-purple/12 border-b border-purple/40 text-[11.5px] text-text-base leading-relaxed whitespace-pre-line"
    >
      {{ SANITIZER_AUDIT_BANNER }}
    </div>

    <!-- Header -->
    <header
      class="flex items-center gap-3 px-4 py-3 bg-surface-2 border-b border-border-soft"
    >
      <div class="flex-1 min-w-0">
        <div class="text-text-base font-semibold text-[13.5px]">
          Sanitizer Audit Inspector
        </div>
        <div
          class="font-mono text-[10.5px] text-text-mute mt-0.5 flex items-center gap-2"
        >
          <span class="text-text-dim">{{ row.ts }}</span>
        </div>
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

    <!-- Status strip with chips -->
    <div
      class="flex flex-wrap items-center gap-2 px-4 py-2 border-b border-border-soft text-[10.5px] font-mono"
    >
      <span
        class="px-2 py-[2px] rounded border border-border-base text-text-dim"
      >
        {{ row.rule_id }}
      </span>
      <span
        data-testid="placeholder-token"
        class="px-2 py-[2px] rounded border border-purple text-purple bg-purple/12"
      >
        {{ placeholderToken }}
      </span>
      <span
        data-testid="pass-label"
        class="px-2 py-[2px] rounded border border-accent text-accent"
      >
        {{ passLabel }}
      </span>
      <span
        v-if="isAllowlisted"
        data-testid="allowlisted-chip"
        class="ml-auto px-2 py-[2px] rounded border border-green text-green"
      >
        ✓ allowlisted
      </span>
    </div>

    <!-- Body -->
    <div class="flex-1 overflow-y-auto px-4 py-3">
      <!-- Metadata table -->
      <dl
        class="grid grid-cols-[140px_1fr] gap-x-4 gap-y-2 font-mono text-[12px]"
      >
        <dt class="text-text-mute">ts</dt>
        <dd class="text-text-base">{{ row.ts }}</dd>
        <dt class="text-text-mute">schema_version</dt>
        <dd class="text-text-base">{{ row.schema_version }}</dd>
        <dt class="text-text-mute">rules_version</dt>
        <dd class="text-text-base">{{ row.rules_version }}</dd>
        <dt class="text-text-mute">pass</dt>
        <dd class="text-text-base">{{ passLabel }}</dd>
        <dt class="text-text-mute">session_id</dt>
        <dd class="text-text-base">{{ row.session_id }}</dd>
        <dt class="text-text-mute">source</dt>
        <dd class="text-text-base">
          <span>{{ sourceView.label }}</span>
          <details
            v-if="sourceView.kind === 'unknown'"
            class="mt-1 text-[10.5px] text-text-dim"
          >
            <summary>raw</summary>
            <pre class="mt-1 whitespace-pre-wrap">{{ rawSourceJson(sourceView) }}</pre>
          </details>
        </dd>
        <dt class="text-text-mute">rule_id</dt>
        <dd class="text-text-base">{{ row.rule_id }}</dd>
        <dt class="text-text-mute">kind</dt>
        <dd class="text-text-base">{{ row.kind }}</dd>
        <dt class="text-text-mute">placeholder_index</dt>
        <dd class="text-text-base">{{ row.placeholder_index }}</dd>
        <dt class="text-text-mute">extra</dt>
        <dd class="text-text-base">
          <div v-if="otherExtraEntries.length === 0 && !isAllowlisted" class="text-text-dim">
            (empty)
          </div>
          <div v-else>
            <div
              v-for="[key, value] in otherExtraEntries"
              :key="key"
              class="text-text-base"
            >
              <span class="text-text-mute">{{ key }}</span>
              <span class="mx-1">:</span>
              <span>{{ String(value) }}</span>
            </div>
            <div v-if="isAllowlisted && otherExtraEntries.length === 0" class="text-text-dim">
              (only allowlisted — see chip above)
            </div>
          </div>
        </dd>
      </dl>

      <!-- Rule explainer -->
      <section
        v-if="ruleEntry !== null"
        data-testid="rule-explainer"
        class="mt-4 px-3 py-3 rounded border border-border-soft bg-surface-2"
      >
        <div class="text-text-mute text-[10.5px] uppercase tracking-[0.16em] mb-1">
          Rule explainer
        </div>
        <div class="text-text-base text-[12px] mb-1">
          {{ ruleEntry.description }}
        </div>
        <div class="font-mono text-[11px] text-text-dim">
          {{ ruleEntry.pattern_summary }}
        </div>
        <div class="mt-1 text-[10.5px] text-text-mute">
          source · {{ ruleEntry.source }}
        </div>
      </section>
      <section
        v-else
        data-testid="rule-explainer-fallback"
        class="mt-4 text-text-dim text-[11px] italic"
      >
        (no rule registry entry for this rule_id — registry may still be loading)
      </section>
    </div>
  </aside>
</template>
