import { useEffect, useState } from "react"

import { useRouteStore } from "@/store/route"

type DragDropPayload = { paths: string[] }
type EventCallback<T> = (event: { payload: T }) => void
type UnlistenFn = () => void

/**
 * Tauri 2 drag-drop listener scoped to the Lobby state. Subscribes to
 * `tauri://drag-enter`, `tauri://drag-over`, `tauri://drag-drop`, and
 * `tauri://drag-leave` so the host can render a drop-target overlay
 * while a folder is being dragged over the window.
 *
 * Returns `isDragOver` — `true` between `drag-enter` and the next
 * `drag-drop` or `drag-leave`. The hook fires `onDrop(firstPath)` on
 * drop; multi-folder drops surface only the first path per spec
 * (`Drop multiple folders picks the first`).
 *
 * The `listenFactory` indirection makes the hook testable without a real
 * Tauri runtime — production wires it to `@tauri-apps/api/event::listen`.
 */
export function useLobbyDragDrop(
  onDrop: (path: string) => void,
  listenFactory?: (
    event: string,
    cb: EventCallback<DragDropPayload>,
  ) => Promise<UnlistenFn>,
): { isDragOver: boolean } {
  const route = useRouteStore((s) => s.route)
  const [isDragOver, setIsDragOver] = useState(false)

  useEffect(() => {
    if (route.kind !== "lobby") {
      setIsDragOver(false)
      return
    }

    let cancelled = false
    const unsubs: UnlistenFn[] = []

    const factory =
      listenFactory ??
      (async (event, cb) => {
        const mod = await import("@tauri-apps/api/event")
        return mod.listen(event, cb)
      })

    const attach = async (
      event: string,
      cb: EventCallback<DragDropPayload>,
    ) => {
      try {
        const fn = await factory(event, cb)
        if (cancelled) {
          fn()
        } else {
          unsubs.push(fn)
        }
      } catch (err) {
        console.error("[drag-drop] failed to attach", event, err)
      }
    }

    void attach("tauri://drag-enter", () => {
      setIsDragOver(true)
    })
    void attach("tauri://drag-over", () => {
      // Repeated event — keep overlay open without state churn.
    })
    void attach("tauri://drag-leave", () => {
      setIsDragOver(false)
    })
    void attach("tauri://drag-drop", (event) => {
      setIsDragOver(false)
      const paths = event.payload?.paths ?? []
      if (paths.length === 0) return
      onDrop(paths[0])
    })

    return () => {
      cancelled = true
      for (const fn of unsubs) {
        try {
          fn()
        } catch {
          // ignore
        }
      }
      setIsDragOver(false)
    }
  }, [route.kind, onDrop, listenFactory])

  return { isDragOver }
}
