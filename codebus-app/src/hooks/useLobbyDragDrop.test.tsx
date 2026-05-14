import { describe, expect, it, vi, beforeEach } from "vitest"
import { render, act } from "@testing-library/react"

import { useLobbyDragDrop } from "./useLobbyDragDrop"
import { useRouteStore } from "@/store/route"

function Harness({
  onDrop,
  factory,
  onState,
}: {
  onDrop: (p: string) => void
  factory: ReturnType<typeof makeFactory>["factory"]
  onState?: (s: { isDragOver: boolean }) => void
}) {
  const state = useLobbyDragDrop(onDrop, factory)
  onState?.(state)
  return null
}

function makeFactory() {
  // Track every subscribed event so tests can drive enter/leave/drop
  // separately, not just the last call.
  const registered = new Map<
    string,
    (event: { payload: { paths: string[] } }) => void
  >()
  const unlisten = vi.fn()
  const factory = vi.fn(
    async (
      event: string,
      cb: (event: { payload: { paths: string[] } }) => void,
    ) => {
      registered.set(event, cb)
      return unlisten
    },
  )
  return {
    factory,
    fire(event: string, paths: string[]) {
      registered.get(event)?.({ payload: { paths } })
    },
    unlisten,
    isRegistered() {
      return registered.size > 0
    },
  }
}

describe("useLobbyDragDrop", () => {
  beforeEach(() => {
    useRouteStore.setState({ route: { kind: "lobby" } })
  })

  it("subscribes to tauri://drag-drop while route is lobby", async () => {
    const f = makeFactory()
    const onDrop = vi.fn()
    render(<Harness onDrop={onDrop} factory={f.factory} />)
    await new Promise((r) => setTimeout(r, 0))
    expect(f.factory).toHaveBeenCalledWith(
      "tauri://drag-drop",
      expect.any(Function),
    )
  })

  it("dragdrop_disabled_in_workspace_state", async () => {
    useRouteStore.setState({
      route: {
        kind: "workspace",
        vault: {
          path: "/v",
          display_name: "v",
          last_opened: "2026-05-11T00:00:00Z",
          is_missing: false,
        },
      },
    })
    const f = makeFactory()
    const onDrop = vi.fn()
    render(<Harness onDrop={onDrop} factory={f.factory} />)
    await new Promise((r) => setTimeout(r, 0))
    expect(f.factory).not.toHaveBeenCalled()
  })

  it("invokes onDrop with the first path when multiple folders dropped", async () => {
    const f = makeFactory()
    const onDrop = vi.fn()
    render(<Harness onDrop={onDrop} factory={f.factory} />)
    await new Promise((r) => setTimeout(r, 0))
    act(() => {
      f.fire("tauri://drag-drop", ["/first", "/second", "/third"])
    })
    expect(onDrop).toHaveBeenCalledTimes(1)
    expect(onDrop).toHaveBeenCalledWith("/first")
  })

  it("ignores empty drop payload (no path)", async () => {
    const f = makeFactory()
    const onDrop = vi.fn()
    render(<Harness onDrop={onDrop} factory={f.factory} />)
    await new Promise((r) => setTimeout(r, 0))
    act(() => {
      f.fire("tauri://drag-drop", [])
    })
    expect(onDrop).not.toHaveBeenCalled()
  })

  it("isDragOver flips true on enter, false on leave", async () => {
    const f = makeFactory()
    let latest: { isDragOver: boolean } = { isDragOver: false }
    render(
      <Harness
        onDrop={() => {}}
        factory={f.factory}
        onState={(s) => {
          latest = s
        }}
      />,
    )
    await new Promise((r) => setTimeout(r, 0))
    expect(latest.isDragOver).toBe(false)

    act(() => {
      f.fire("tauri://drag-enter", ["/x"])
    })
    expect(latest.isDragOver).toBe(true)

    act(() => {
      f.fire("tauri://drag-leave", [])
    })
    expect(latest.isDragOver).toBe(false)
  })

  it("isDragOver resets to false after a drop", async () => {
    const f = makeFactory()
    let latest: { isDragOver: boolean } = { isDragOver: false }
    render(
      <Harness
        onDrop={() => {}}
        factory={f.factory}
        onState={(s) => {
          latest = s
        }}
      />,
    )
    await new Promise((r) => setTimeout(r, 0))

    act(() => {
      f.fire("tauri://drag-enter", ["/x"])
    })
    expect(latest.isDragOver).toBe(true)

    act(() => {
      f.fire("tauri://drag-drop", ["/x"])
    })
    expect(latest.isDragOver).toBe(false)
  })
})
