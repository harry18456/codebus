/**
 * `useWatcherEvent` — the single frontend entry point for subscribing to
 * filesystem-watcher Tauri events defined by the `fs-watcher` capability.
 *
 * Spec contract (`Frontend useWatcherEvent Hook`):
 *   - Wraps `@tauri-apps/api/event::listen` so direct `listen("wiki-...")`
 *     calls do NOT appear elsewhere in the frontend.
 *   - Returns a synchronous cleanup function suitable for use as a
 *     `useEffect` return value.
 *
 * Usage:
 * ```tsx
 * useEffect(
 *   () => useWatcherEvent("wiki-list-changed", () => store.listPages()),
 *   [],
 * )
 * ```
 *
 * Implementation note: `listen()` is async because Tauri sets the
 * subscription up on the Rust side. We capture the eventual unlisten
 * handle in a closure and apply it lazily — if the caller invokes the
 * returned cleanup before the listen promise resolves, we set a
 * `cancelled` flag and unlisten as soon as the handle arrives.
 */
import { listen, type UnlistenFn } from "@tauri-apps/api/event"

/** Watcher event names emitted by the `codebus-app` Rust backend. */
export type WatcherEventName =
  | "wiki-list-changed"
  | "wiki-page-changed"
  | "goals-changed"
  | "goal-run-changed"
  | "quiz-changed"
  | "quiz-attempt-changed"
  | "vault-list-changed"
  | "vault-watcher-error"

/** Typed payloads matching `EmitKind::payload` on the Rust side. */
export interface WatcherEventPayloads {
  "wiki-list-changed": null
  "wiki-page-changed": { path: string }
  "goals-changed": null
  "goal-run-changed": { run_id: string }
  "quiz-changed": null
  "quiz-attempt-changed": { slug: string; id: string }
  "vault-list-changed": null
  "vault-watcher-error": { vault_path: string; reason: string }
}

/**
 * Subscribe to a watcher Tauri event. Returns a cleanup function that
 * unsubscribes the listener. Safe to call the cleanup synchronously even
 * before the underlying `listen()` promise has resolved.
 */
export function useWatcherEvent<E extends WatcherEventName>(
  eventName: E,
  handler: (payload: WatcherEventPayloads[E]) => void,
): () => void {
  let cancelled = false
  let unlisten: UnlistenFn | undefined

  void listen<WatcherEventPayloads[E]>(eventName, (event) => {
    handler(event.payload)
  }).then((fn) => {
    if (cancelled) {
      fn()
    } else {
      unlisten = fn
    }
  })

  return () => {
    cancelled = true
    unlisten?.()
  }
}
