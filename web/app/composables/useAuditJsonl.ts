import { ref, watch, type Ref } from 'vue'
import type { SseEvent } from './useSseTask'
import type { UseExplorerStreamApi } from './useExplorerStream'
import type { UseQaSessionApi } from './useQaSession'

// useAuditJsonl — typed wrapper for the Tauri `read_audit_jsonl` IPC
// command. Loads `<workspace_root>/.codebus/<file>.jsonl` once, then
// optionally tails new entries from a live SSE source. The composable
// MUST NOT instantiate its own EventSource — live-tail piggybacks on a
// caller-provided `useExplorerStream` instance (or, in qa-overlay-p0,
// `useQaSession`).

export type AuditKind =
  | 'sanitize'
  | 'tool'
  | 'reasoning'
  | 'token'
  | 'llm'
  | 'kb_growth'
  | 'generator'

export interface LlmCallEntry {
  timestamp: string
  request_id?: string
  role: 'reasoning' | 'judge' | 'chat' | 'embed' | 'pii_detection'
  module?: string
  provider_id: string
  model: string
  call_type?: string
  prompt_tokens: number
  completion_tokens: number
  cost_usd?: number | null
  latency_ms?: number | null
  sanitizer_pass2_applied: boolean
  request: Record<string, unknown>
  response: Record<string, unknown> | null
  error?: { class: string; message: string }
}

export interface UseAuditJsonlOptions {
  liveTailFromExplorerStream?: UseExplorerStreamApi
  // qa-overlay-p0: kb_growth tab live-tails from useQaSession's SSE chain
  // mirroring the explorer-stream pattern. Dedup key is `entry_id`.
  liveTailFromQaSession?: UseQaSessionApi
}

export interface UseAuditJsonlApi<T = Record<string, unknown>> {
  entries: Ref<T[]>
  loading: Ref<boolean>
  error: Ref<Error | null>
  reload: () => Promise<void>
}

// Cache the dynamic import promise so concurrent reload() calls share
// one resolution. Without this, two parallel `await import(...)` against
// the Vitest mock can race and yield `undefined` for the second caller.
let _coreModulePromise: Promise<typeof import('@tauri-apps/api/core')> | null = null
function loadCore(): Promise<typeof import('@tauri-apps/api/core')> {
  if (_coreModulePromise === null) {
    _coreModulePromise = import('@tauri-apps/api/core')
  }
  return _coreModulePromise
}

async function tauriInvoke<T>(
  cmd: string,
  args: Record<string, unknown>
): Promise<T> {
  const { invoke } = await loadCore()
  return invoke<T>(cmd, args)
}

function asEntry<T = Record<string, unknown>>(value: unknown): T {
  return value as T
}

function asString(value: unknown): string | undefined {
  return typeof value === 'string' ? value : undefined
}

function getRequestId(entry: unknown): string | undefined {
  if (entry && typeof entry === 'object' && 'request_id' in entry) {
    return asString((entry as { request_id: unknown }).request_id)
  }
  return undefined
}

function getEntryId(entry: unknown): string | undefined {
  if (entry && typeof entry === 'object' && 'entry_id' in entry) {
    return asString((entry as { entry_id: unknown }).entry_id)
  }
  return undefined
}

export function useAuditJsonl<T = Record<string, unknown>>(
  workspaceRoot: string,
  kind: AuditKind,
  opts: UseAuditJsonlOptions = {}
): UseAuditJsonlApi<T> {
  const entries: Ref<T[]> = ref([]) as Ref<T[]>
  const loading: Ref<boolean> = ref(true)
  const error: Ref<Error | null> = ref(null)

  async function reload(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      const result = await tauriInvoke<unknown[]>('read_audit_jsonl', {
        workspaceRoot,
        auditKind: kind
      })
      entries.value = result.map((e) => asEntry<T>(e))
    } catch (err) {
      const message = typeof err === 'string' ? err : err instanceof Error ? err.message : String(err)
      error.value = new Error(message)
      entries.value = []
    } finally {
      loading.value = false
    }
  }

  // Initial load — fire-and-forget; consumers wait via `loading`.
  void reload()

  // Live-tail wiring: only `kind === 'llm'` consumes
  // `liveTailFromExplorerStream`'s `llm_call` events. Other kinds
  // ignore the option silently per spec.
  if (kind === 'llm' && opts.liveTailFromExplorerStream) {
    const stream = opts.liveTailFromExplorerStream
    // Tap the SSE event ref. Production: the public `events` surface
    // forwarded from useSseTask. Tests: a `__sseEvents` bag attached
    // by the fake stream factory; both shapes are interchangeable.
    const eventsRef =
      stream.events ??
      (stream as unknown as { __sseEvents?: Ref<SseEvent[]> }).__sseEvents
    if (eventsRef) {
      let cursor = eventsRef.value.length
      watch(
        () => eventsRef.value.length,
        (len) => {
          while (cursor < len) {
            const ev = eventsRef.value[cursor]
            cursor += 1
            if (ev?.type !== 'llm_call') continue
            const data = ev.data as Record<string, unknown> | undefined
            if (!data) continue
            const requestId = getRequestId(data)
            if (
              requestId &&
              entries.value.some((e) => getRequestId(e) === requestId)
            ) {
              continue
            }
            entries.value.push(asEntry<T>(data))
          }
        },
        { immediate: false }
      )
    }
  }

  // Live-tail wiring (qa-overlay-p0): only `kind === 'kb_growth'` consumes
  // `liveTailFromQaSession`'s `kb_growth` events. Dedup by `entry_id`.
  if (kind === 'kb_growth' && opts.liveTailFromQaSession) {
    const session = opts.liveTailFromQaSession
    // Production: useQaSession exposes its inner SSE event chain via the
    // private `__sseEvents` slot (analogous to useExplorerStream's hook).
    // Tests pass the same slot inline so live-tail works identically.
    const eventsRef = (
      session as unknown as { __sseEvents?: Ref<SseEvent[]> }
    ).__sseEvents
    if (eventsRef) {
      let cursor = eventsRef.value.length
      watch(
        () => eventsRef.value.length,
        (len) => {
          while (cursor < len) {
            const ev = eventsRef.value[cursor]
            cursor += 1
            if (ev?.type !== 'kb_growth') continue
            const data = ev.data as Record<string, unknown> | undefined
            if (!data) continue
            const entryId = getEntryId(data)
            if (
              entryId &&
              entries.value.some((e) => getEntryId(e) === entryId)
            ) {
              continue
            }
            entries.value.push(asEntry<T>(data))
          }
        },
        { immediate: false }
      )
    }
  }

  return { entries, loading, error, reload }
}
