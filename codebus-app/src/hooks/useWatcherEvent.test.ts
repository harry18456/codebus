import { describe, expect, it, vi, beforeEach } from "vitest"

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}))

import { listen } from "@tauri-apps/api/event"
import { useWatcherEvent } from "./useWatcherEvent"

const mockedListen = vi.mocked(listen)

describe("useWatcherEvent", () => {
  beforeEach(() => {
    mockedListen.mockReset()
  })

  it("subscribes_and_cleans_up", async () => {
    const unlistenSpy = vi.fn()
    mockedListen.mockResolvedValueOnce(unlistenSpy)

    const handler = vi.fn()
    const cleanup = useWatcherEvent("wiki-list-changed", handler)

    // Listener was registered with the correct event name.
    expect(mockedListen).toHaveBeenCalledTimes(1)
    expect(mockedListen.mock.calls[0]?.[0]).toBe("wiki-list-changed")

    // Wait for the listen promise to resolve so the unlisten handle is
    // captured by the hook.
    await Promise.resolve()
    await Promise.resolve()

    // Cleanup unsubscribes via the captured unlisten function.
    cleanup()
    expect(unlistenSpy).toHaveBeenCalledTimes(1)
  })

  it("cleanup invoked before listen resolves still unsubscribes", async () => {
    const unlistenSpy = vi.fn()
    // Mock listen to defer resolution so cleanup races ahead.
    let resolveListen: ((fn: () => void) => void) | undefined
    mockedListen.mockImplementationOnce(
      () =>
        new Promise((res) => {
          resolveListen = res as (fn: () => void) => void
        }),
    )

    const cleanup = useWatcherEvent("wiki-page-changed", () => {})
    cleanup() // before listen resolves

    // Now resolve the listen promise.
    resolveListen?.(unlistenSpy)
    await Promise.resolve()
    await Promise.resolve()

    // The unlisten function SHALL still be invoked since cancellation
    // was recorded before the handle arrived.
    expect(unlistenSpy).toHaveBeenCalledTimes(1)
  })

  it("handler receives typed payload from the Tauri event", async () => {
    let capturedCallback:
      | ((ev: { payload: { path: string } }) => void)
      | undefined
    const unlistenSpy = vi.fn()
    mockedListen.mockImplementationOnce(async (_name, cb) => {
      capturedCallback = cb as (ev: { payload: { path: string } }) => void
      return unlistenSpy
    })

    const handler = vi.fn()
    useWatcherEvent("wiki-page-changed", handler)
    await Promise.resolve()
    await Promise.resolve()

    // Simulate a Tauri event delivery.
    capturedCallback?.({ payload: { path: "C:/vault/.codebus/wiki/foo.md" } })
    expect(handler).toHaveBeenCalledWith({
      path: "C:/vault/.codebus/wiki/foo.md",
    })
  })
})
