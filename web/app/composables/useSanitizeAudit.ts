import { computed, type ComputedRef, type Ref } from 'vue'
import { useAuditJsonl, type UseAuditJsonlOptions } from './useAuditJsonl'

// useSanitizeAudit — thin wrapper over useAuditJsonl('sanitize') that
// derives view-model fields the SanitizerAuditInspector overlay and the
// AuditPanel sanitize tab both consume. Per spec
// `Requirement: useSanitizeAudit composable parses sanitize_audit rows into a view-model`:
//   * Derives sourceView (parsed source field), placeholderToken (chip text),
//     and passLabel (human-readable mapping)
//   * Exposes reactive derived collections kindSummary + sessionTimeline
//   * Does NOT mount HTTP listeners, hold its own polling timer, or talk
//     to Tauri IPC — those concerns belong to the underlying useAuditJsonl
//     dependency. The defensive source-grep test asserts no Tauri command
//     name appears literally at this call site.
//   * Does NOT touch useSanitizerRules — rule explainer lookup is a
//     parent-layer concern (the inspector's responsibility)
//
// Forward-reference (P1+) — `sanitizer-audit-unlock` will own raw retention.
// This composable stays metadata-only and adds no fields that depend on
// pre-sanitize text.

export type SanitizePassNum = 1 | 2 | 3

export const PASS_LABELS: Record<SanitizePassNum, string> = {
  1: 'Pass 1 · Scanner (KB ingestion)',
  2: 'Pass 2 · Provider pre-flight (LLM call)',
  3: 'Pass 3 · Q&A add_to_kb'
}

export interface SanitizeAuditEntry {
  ts: string
  schema_version: number
  rules_version: string
  pass: number
  session_id: string
  source: string | { pass?: string; path?: string; [k: string]: unknown }
  rule_id: string
  kind: string
  placeholder_index: number
  extra: Record<string, unknown>
}

export type SanitizeSourceView =
  | { kind: 'file'; pass: string | null; path: string; label: string }
  | { kind: 'message'; pass: null; message_id: string; label: string }
  | { kind: 'unknown'; pass: null; label: string; raw: unknown }

export interface SanitizeRowView {
  row: SanitizeAuditEntry
  sourceView: SanitizeSourceView
  placeholderToken: string
  passLabel: string
}

export interface UseSanitizeAuditApi {
  entries: Ref<SanitizeAuditEntry[]>
  rowViews: ComputedRef<SanitizeRowView[]>
  kindSummary: ComputedRef<Map<string, number>>
  sessionTimeline: ComputedRef<Map<string, SanitizeRowView[]>>
  loading: Ref<boolean>
  error: Ref<Error | null>
  reload: () => Promise<void>
}

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
      return {
        kind: 'file',
        pass: passValue,
        path: pathValue,
        label
      }
    }
  }
  return { kind: 'unknown', pass: null, label: '(unknown source format)', raw }
}

function passLabelFor(pass: number): string {
  return PASS_LABELS[pass as SanitizePassNum] ?? String(pass)
}

function placeholderToken(row: SanitizeAuditEntry): string {
  return `<REDACTED:${row.kind}#${row.placeholder_index}>`
}

function rowToView(row: SanitizeAuditEntry): SanitizeRowView {
  return {
    row,
    sourceView: parseSource(row.source),
    placeholderToken: placeholderToken(row),
    passLabel: passLabelFor(row.pass)
  }
}

export function useSanitizeAudit(
  workspaceRoot: string,
  opts: UseAuditJsonlOptions = {}
): UseSanitizeAuditApi {
  const audit = useAuditJsonl<SanitizeAuditEntry>(workspaceRoot, 'sanitize', opts)

  const rowViews = computed<SanitizeRowView[]>(() =>
    audit.entries.value.map(rowToView)
  )

  const kindSummary = computed<Map<string, number>>(() => {
    const counts = new Map<string, number>()
    for (const row of audit.entries.value) {
      counts.set(row.kind, (counts.get(row.kind) ?? 0) + 1)
    }
    return counts
  })

  const sessionTimeline = computed<Map<string, SanitizeRowView[]>>(() => {
    const grouped = new Map<string, SanitizeRowView[]>()
    for (const view of rowViews.value) {
      const sid = view.row.session_id
      const bucket = grouped.get(sid)
      if (bucket) {
        bucket.push(view)
      } else {
        grouped.set(sid, [view])
      }
    }
    for (const bucket of grouped.values()) {
      bucket.sort((a, b) => a.row.ts.localeCompare(b.row.ts))
    }
    return grouped
  })

  return {
    entries: audit.entries,
    rowViews,
    kindSummary,
    sessionTimeline,
    loading: audit.loading,
    error: audit.error,
    reload: audit.reload
  }
}
