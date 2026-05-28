import { describe, expect, it, vi, beforeEach, afterEach } from "vitest"
import { act, render, screen } from "@testing-library/react"

// `listen` mock is captured per-test; the factory below re-reads
// `listenSpy` at call time so individual tests can swap behavior.
const listenSpy = vi.fn<
  (event: string, handler: (e: { payload: unknown }) => void) => Promise<() => void>
>()
const unlistenSpy = vi.fn<() => void>()

vi.mock("@tauri-apps/api/event", () => ({
  listen: (event: string, handler: (e: { payload: unknown }) => void) =>
    listenSpy(event, handler),
}))

import { LoadingOverlay } from "./LoadingOverlay"
import { useVaultsStore } from "@/store/vaults"

/** Reset zustand store + spies between tests. */
function resetState() {
  useVaultsStore.setState({
    vaults: [],
    loading: false,
    initInProgress: true,
    initFailedError: null,
    lastAddVaultArgs: null,
    error: null,
  })
  listenSpy.mockReset()
  unlistenSpy.mockReset()
  listenSpy.mockImplementation(async () => unlistenSpy)
}

describe("LoadingOverlay", () => {
  beforeEach(() => {
    vi.useFakeTimers()
    resetState()
  })

  afterEach(() => {
    vi.runOnlyPendingTimers()
    vi.useRealTimers()
  })

  it("Initial mount shows fallback before any event (phase=0)", async () => {
    render(<LoadingOverlay />)
    // Wait one tick so the listen subscription resolves.
    await act(async () => {
      await Promise.resolve()
    })

    // Fallback path renders v1 loading.title + loading.subtitle, no dots.
    expect(screen.getByTestId("loading-overlay")).toBeInTheDocument()
    // title is the v1 key; both fallback and phase>=1 share it.
    expect(screen.getByRole("heading")).toBeInTheDocument()
    expect(
      screen.queryByTestId("loading-overlay-phase-dots"),
    ).not.toBeInTheDocument()
  })

  it("Phase advances on event (dots appear, bus DOM node identity stable)", async () => {
    const { container } = render(<LoadingOverlay />)
    await act(async () => {
      await Promise.resolve()
    })

    const busBefore = container.querySelector("[aria-hidden='true']")
    expect(busBefore).not.toBeNull()

    // Capture the handler that LoadingOverlay registered with listen().
    expect(listenSpy).toHaveBeenCalledWith(
      "vault-init-progress",
      expect.any(Function),
    )
    const handler = listenSpy.mock.calls[0][1] as (e: {
      payload: unknown
    }) => void

    await act(async () => {
      handler({
        payload: { phase: 1, init_event_kind: "Start", elapsed_ms: 5 },
      })
    })

    // Phase dots now visible.
    const dots = screen.getByTestId("loading-overlay-phase-dots")
    expect(dots).toBeInTheDocument()
    expect(dots.getAttribute("data-phase")).toBe("1")

    // Bus DOM node identity is preserved across the phase transition
    // (per spec scenario "Phase advances on event").
    const busAfter = container.querySelector("[aria-hidden='true']")
    expect(busAfter).toBe(busBefore)
  })

  it("Backend skips phase 5 but UI still pauses (minimum 300ms)", async () => {
    render(<LoadingOverlay />)
    await act(async () => {
      await Promise.resolve()
    })
    const handler = listenSpy.mock.calls[0][1] as (e: {
      payload: unknown
    }) => void

    // Backend bursts phases 4 → 5 → 6 within ~100ms. UI stepped through
    // each in order with 300ms residence so dots fill smoothly.
    await act(async () => {
      handler({
        payload: { phase: 4, init_event_kind: "SettingsDone", elapsed_ms: 100 },
      })
    })
    // First phase commits sync → phase 1 visible.
    expect(screen.getByTestId("loading-overlay-phase-dots").getAttribute("data-phase")).toBe("1")

    // Step to 2 / 3 / 4 (each takes 300ms).
    await act(async () => { vi.advanceTimersByTime(310) })
    expect(screen.getByTestId("loading-overlay-phase-dots").getAttribute("data-phase")).toBe("2")
    await act(async () => { vi.advanceTimersByTime(310) })
    expect(screen.getByTestId("loading-overlay-phase-dots").getAttribute("data-phase")).toBe("3")
    await act(async () => { vi.advanceTimersByTime(310) })
    expect(screen.getByTestId("loading-overlay-phase-dots").getAttribute("data-phase")).toBe("4")

    // Backend now bursts 5 and 6.
    await act(async () => {
      handler({
        payload: { phase: 5, init_event_kind: "ObsidianSkipped", elapsed_ms: 150 },
      })
      handler({
        payload: { phase: 6, init_event_kind: "CommitDone", elapsed_ms: 200 },
      })
    })
    // Phase 4 still visible (elapsed since phase 4 entry < 300ms).
    expect(screen.getByTestId("loading-overlay-phase-dots").getAttribute("data-phase")).toBe("4")

    // 300ms later → phase 5 takes over.
    await act(async () => { vi.advanceTimersByTime(310) })
    expect(screen.getByTestId("loading-overlay-phase-dots").getAttribute("data-phase")).toBe("5")

    // Another 300ms → phase 6 (proves phase 5 had its slot even though
    // backend skipped directly to 6).
    await act(async () => { vi.advanceTimersByTime(310) })
    expect(screen.getByTestId("loading-overlay-phase-dots").getAttribute("data-phase")).toBe("6")
  })

  it("Backend error enters failure mode (amber-warm + retry button)", async () => {
    render(<LoadingOverlay />)
    await act(async () => {
      await Promise.resolve()
    })
    const handler = listenSpy.mock.calls[0][1] as (e: {
      payload: unknown
    }) => void
    await act(async () => {
      handler({
        payload: { phase: 3, init_event_kind: "NestedRepoDone", elapsed_ms: 80 },
      })
      vi.advanceTimersByTime(350)
    })

    // Step up to phase 3 so failure carries that as the last reached phase.
    await act(async () => { vi.advanceTimersByTime(310) })
    await act(async () => { vi.advanceTimersByTime(310) })
    expect(screen.getByTestId("loading-overlay-phase-dots").getAttribute("data-phase")).toBe("3")

    // Simulate the store catching an init error.
    await act(async () => {
      useVaultsStore.setState({
        initInProgress: false,
        initFailedError: { key: "errors.io", vars: { message: "permission denied" } },
        lastAddVaultArgs: { path: "/tmp/repo", mode: "detect" },
      })
    })

    // Failure UI: title swapped, retry button visible, current dot is error.
    expect(screen.getByTestId("loading-overlay-retry")).toBeInTheDocument()
    const dots = screen.getByTestId("loading-overlay-phase-dots")
    const active = dots.querySelector("[data-phase-state='error']")
    expect(active).not.toBeNull()
    expect(active?.getAttribute("data-phase-index")).toBe("3")
  })

  it("Slow phase shows dim hint after 20s without an event", async () => {
    render(<LoadingOverlay />)
    await act(async () => {
      await Promise.resolve()
    })
    const handler = listenSpy.mock.calls[0][1] as (e: {
      payload: unknown
    }) => void

    await act(async () => {
      handler({
        payload: { phase: 4, init_event_kind: "SchemaDone", elapsed_ms: 100 },
      })
    })
    // Step up to phase 4 first.
    await act(async () => { vi.advanceTimersByTime(310) })
    await act(async () => { vi.advanceTimersByTime(310) })
    await act(async () => { vi.advanceTimersByTime(310) })

    expect(screen.queryByTestId("loading-overlay-slow-hint")).not.toBeInTheDocument()

    await act(async () => {
      vi.advanceTimersByTime(20_500)
    })
    expect(screen.getByTestId("loading-overlay-slow-hint")).toBeInTheDocument()

    // Next event advances phase → hint disappears.
    await act(async () => {
      handler({
        payload: { phase: 5, init_event_kind: "ObsidianResult", elapsed_ms: 21_000 },
      })
    })
    await act(async () => { vi.advanceTimersByTime(400) })
    expect(screen.queryByTestId("loading-overlay-slow-hint")).not.toBeInTheDocument()
  })

  it("Backend never emits events but IPC succeeds → no error UI", async () => {
    render(<LoadingOverlay />)
    await act(async () => {
      await Promise.resolve()
    })

    // Simulate init success without any progress event — initInProgress
    // flips false directly (overlay was in fallback phase 0 the whole
    // time, no queued phases).
    await act(async () => {
      useVaultsStore.setState({
        initInProgress: false,
        initFailedError: null,
        lastAddVaultArgs: null,
      })
    })
    // No retry button, no failure title. Overlay returns null in fully
    // idle state (latestKnownPhase still 0, no fade-out since no phase
    // was ever displayed).
    expect(screen.queryByTestId("loading-overlay-retry")).not.toBeInTheDocument()
  })

  it("Listener is detached on unmount", async () => {
    const { unmount } = render(<LoadingOverlay />)
    await act(async () => {
      await Promise.resolve()
    })
    unmount()
    // Allow the cleanup promise (resolved unlisten) to flush.
    await act(async () => {
      await Promise.resolve()
    })
    expect(unlistenSpy).toHaveBeenCalled()
  })
})
