// Vitest global setup for web/. Bootstrapped by change agent-console-p0.
//
// happy-dom does not ship an EventSource implementation; the SSE composable
// relies on the global. We register a controllable FakeEventSource that
// individual tests can drive via `instance._emit(type, data)`.

import { afterEach, beforeEach } from 'vitest'

type EventSourceListener = (event: MessageEvent<string>) => void

export interface FakeEventSourceInstance {
  url: string
  withCredentials: boolean
  readyState: number
  onopen: ((event: Event) => void) | null
  onerror: ((event: Event) => void) | null
  onmessage: EventSourceListener | null
  close: () => void
  // test-only hooks (prefixed with `_` to flag they are not part of the
  // browser EventSource API surface)
  _emit: (type: string, data: unknown) => void
  _emitMessage: (data: unknown) => void
  _simulateOpen: () => void
  _simulateError: () => void
}

declare global {
  // eslint-disable-next-line no-var
  var __FAKE_ES_INSTANCES__: FakeEventSourceInstance[]
}

class FakeEventSource implements FakeEventSourceInstance {
  static readonly CONNECTING = 0
  static readonly OPEN = 1
  static readonly CLOSED = 2

  readonly CONNECTING = 0
  readonly OPEN = 1
  readonly CLOSED = 2

  url: string
  withCredentials: boolean
  readyState: number = 0

  onopen: ((event: Event) => void) | null = null
  onerror: ((event: Event) => void) | null = null
  onmessage: EventSourceListener | null = null

  private namedListeners = new Map<string, Set<EventSourceListener>>()

  constructor(url: string, init?: { withCredentials?: boolean }) {
    this.url = url
    this.withCredentials = init?.withCredentials ?? false
    globalThis.__FAKE_ES_INSTANCES__.push(this)
  }

  addEventListener(type: string, listener: EventSourceListener): void {
    let set = this.namedListeners.get(type)
    if (!set) {
      set = new Set()
      this.namedListeners.set(type, set)
    }
    set.add(listener)
  }

  removeEventListener(type: string, listener: EventSourceListener): void {
    this.namedListeners.get(type)?.delete(listener)
  }

  dispatchEvent(_event: Event): boolean {
    return true
  }

  close(): void {
    this.readyState = this.CLOSED
  }

  _simulateOpen(): void {
    this.readyState = this.OPEN
    this.onopen?.(new Event('open'))
  }

  _simulateError(): void {
    // Real EventSource dispatches connection-level errors to BOTH the
    // `.onerror` IDL attribute AND any `addEventListener('error', ...)`
    // listeners. Mirroring that here so tests can assert the
    // `useSseTask` listener correctly distinguishes connection errors
    // (generic Event) from server-emitted `event: error` SSE messages
    // (MessageEvent), per `sidecar-sse-named-events-and-error-listener-fix`
    // spec scenario "Named error listener ignores connection-level errors".
    const ev = new Event('error')
    this.onerror?.(ev)
    const listeners = this.namedListeners.get('error')
    if (listeners) {
      // Cast: the listener type accepts MessageEvent for typing
      // convenience, but addEventListener('error') in the real DOM is
      // invoked with whatever Event subclass the runtime dispatches.
      for (const l of listeners) (l as unknown as (e: Event) => void)(ev)
    }
  }

  _emit(type: string, data: unknown): void {
    const payload = typeof data === 'string' ? data : JSON.stringify(data)
    const ev = new MessageEvent<string>(type, { data: payload })
    const listeners = this.namedListeners.get(type)
    if (listeners) {
      for (const l of listeners) l(ev)
    }
  }

  _emitMessage(data: unknown): void {
    const payload = typeof data === 'string' ? data : JSON.stringify(data)
    const ev = new MessageEvent<string>('message', { data: payload })
    this.onmessage?.(ev)
  }
}

// Nuxt compile-time macros are auto-imported in production but absent
// under vitest. Stub them at module load so files that reference them
// at top-level (route middleware, pages with `definePageMeta`) can be
// imported without a `ReferenceError`.
;(globalThis as unknown as { definePageMeta: (meta: unknown) => void }).definePageMeta =
  () => undefined
;(globalThis as unknown as {
  defineNuxtRouteMiddleware: <T>(fn: T) => T
}).defineNuxtRouteMiddleware = (fn) => fn
;(globalThis as unknown as { navigateTo: (target: string) => string }).navigateTo =
  (target) => target

beforeEach(() => {
  globalThis.__FAKE_ES_INSTANCES__ = []
  ;(globalThis as unknown as { EventSource: typeof FakeEventSource }).EventSource =
    FakeEventSource
})

afterEach(() => {
  for (const inst of globalThis.__FAKE_ES_INSTANCES__) inst.close()
  globalThis.__FAKE_ES_INSTANCES__ = []
})

export function getOpenedEventSources(): FakeEventSourceInstance[] {
  return globalThis.__FAKE_ES_INSTANCES__
}

export function lastEventSource(): FakeEventSourceInstance {
  const list = globalThis.__FAKE_ES_INSTANCES__
  if (list.length === 0) throw new Error('No FakeEventSource instances opened')
  return list[list.length - 1]!
}

export { FakeEventSource }
