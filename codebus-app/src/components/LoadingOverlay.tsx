import { useEffect, useMemo, useRef, useState } from "react"

import { listen, type UnlistenFn } from "@tauri-apps/api/event"

import { PhaseDots } from "@/components/PhaseDots"
import type { MessageKey } from "@/i18n/messages"
import { useT } from "@/i18n/useT"
import { useRouteStore } from "@/store/route"
import { useVaultsStore } from "@/store/vaults"

/** Payload of the `vault-init-progress` Tauri event. Mirrors the Rust
 *  struct `codebus_app_tauri::ipc::vault_progress::VaultInitProgress`. */
interface VaultInitProgress {
  phase: number
  init_event_kind: string
  elapsed_ms: number
}

const PHASE_TITLE_KEYS: Record<1 | 2 | 3 | 4 | 5 | 6, MessageKey> = {
  1: "loading.phase.1.title",
  2: "loading.phase.2.title",
  3: "loading.phase.3.title",
  4: "loading.phase.4.title",
  5: "loading.phase.5.title",
  6: "loading.phase.6.title",
}

/** Minimum render time per phase so backend bursts don't flash subtitles
 *  past the user faster than they can read. Per design decision
 *  "Minimum 300ms per phase (所有 phase 同 strategy)". */
const MIN_PHASE_MS = 300
/** Threshold for showing the "this is taking a while" hint. Per spec
 *  scenario "Slow phase shows dim hint". */
const SLOW_PHASE_MS = 20_000
/** Fade-out duration after the queue drains. Per spec scenario
 *  "Successful finish fades out". */
const FADE_OUT_MS = 200

/**
 * Full-window overlay shown while `addVault` is running an init-heavy
 * branch. Listens to the `vault-init-progress` Tauri event and renders
 * a 6-phase live-progress state machine; falls back to the v1 static
 * subtitle when the backend hasn't emitted any event yet, and switches
 * to an amber-warm failure UI when the init IPC rejects.
 *
 * Always-mounted: the component subscribes to `vault-init-progress` for
 * the lifetime of the app and renders DOM only while there is work to
 * show (init in progress, queued phases to display, failure mode, or a
 * fade-out in flight). This decouples the visible "drain the queue"
 * tail from the store's `initInProgress` flag — fast backends that
 * burst-emit all 6 phases within ~100ms still get the full 1.8s
 * step-through display.
 *
 * z-50 so the frameless WindowControls (z-60) stay clickable.
 */
export function LoadingOverlay() {
  const t = useT()
  const initFailedError = useVaultsStore((s) => s.initFailedError)
  const initInProgress = useVaultsStore((s) => s.initInProgress)
  const lastAddVaultArgs = useVaultsStore((s) => s.lastAddVaultArgs)
  const addVault = useVaultsStore((s) => s.addVault)
  const clearInitFailure = useVaultsStore((s) => s.clearInitFailure)
  const openRoute = useRouteStore((s) => s.open)

  const [displayedPhase, setDisplayedPhase] = useState<number>(0)
  const [latestKnownPhase, setLatestKnownPhase] = useState<number>(0)
  const [slowHint, setSlowHint] = useState<boolean>(false)
  const [fadingOut, setFadingOut] = useState<boolean>(false)

  // Refs so async pump callbacks read the freshest values.
  const phaseEnteredAtRef = useRef<number>(0)
  const displayedPhaseRef = useRef<number>(0)
  const latestKnownPhaseRef = useRef<number>(0)
  const pumpTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const slowTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const fadeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // ---- Subscribe to vault-init-progress for the app lifetime ----
  useEffect(() => {
    let unlisten: UnlistenFn | undefined
    let cancelled = false
    void listen<VaultInitProgress>("vault-init-progress", (event) => {
      if (cancelled) return
      const next = event.payload?.phase ?? 0
      if (next < 1 || next > 6) return
      // Track the highest phase the backend has reached; the pump steps
      // the displayed phase up to this ceiling one tick at a time.
      if (next > latestKnownPhaseRef.current) {
        latestKnownPhaseRef.current = next
        setLatestKnownPhase(next)
        pump()
      }
    }).then((fn) => {
      if (cancelled) {
        fn()
      } else {
        unlisten = fn
      }
    })
    return () => {
      cancelled = true
      if (unlisten) unlisten()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  /** Advance the displayed phase by 1, respecting the min-300ms residence
   *  time. If more phases are pending, schedules the next step. The
   *  first phase commits synchronously so the very first event paints
   *  immediately. */
  function pump() {
    if (pumpTimerRef.current) return // already scheduled
    if (displayedPhaseRef.current >= latestKnownPhaseRef.current) return

    if (displayedPhaseRef.current === 0) {
      // First phase: commit synchronously (no setTimeout delay).
      const next = 1
      displayedPhaseRef.current = next
      phaseEnteredAtRef.current = Date.now()
      setDisplayedPhase(next)
      if (displayedPhaseRef.current < latestKnownPhaseRef.current) {
        pump()
      }
      return
    }

    const elapsed = Date.now() - phaseEnteredAtRef.current
    const delay = Math.max(0, MIN_PHASE_MS - elapsed)
    pumpTimerRef.current = setTimeout(() => {
      pumpTimerRef.current = null
      const next = displayedPhaseRef.current + 1
      displayedPhaseRef.current = next
      phaseEnteredAtRef.current = Date.now()
      setDisplayedPhase(next)
      if (displayedPhaseRef.current < latestKnownPhaseRef.current) {
        pump()
      }
    }, delay)
  }

  // ---- Initialize phaseEnteredAt when a new init cycle begins ----
  useEffect(() => {
    if (initInProgress && displayedPhaseRef.current === 0) {
      phaseEnteredAtRef.current = Date.now()
    }
  }, [initInProgress])

  // ---- Slow phase hint ----
  useEffect(() => {
    setSlowHint(false)
    if (slowTimerRef.current) clearTimeout(slowTimerRef.current)
    if (displayedPhase >= 1 && !initFailedError) {
      slowTimerRef.current = setTimeout(() => setSlowHint(true), SLOW_PHASE_MS)
    }
    return () => {
      if (slowTimerRef.current) clearTimeout(slowTimerRef.current)
    }
  }, [displayedPhase, initFailedError])

  // ---- Drain + fade-out when IPC settles successfully ----
  useEffect(() => {
    if (
      !initInProgress &&
      !initFailedError &&
      displayedPhaseRef.current === latestKnownPhaseRef.current &&
      latestKnownPhaseRef.current > 0 &&
      !fadingOut
    ) {
      // Queue drained, init done — fade out then reset.
      setFadingOut(true)
      fadeTimerRef.current = setTimeout(() => {
        setFadingOut(false)
        setDisplayedPhase(0)
        setLatestKnownPhase(0)
        displayedPhaseRef.current = 0
        latestKnownPhaseRef.current = 0
        phaseEnteredAtRef.current = 0
      }, FADE_OUT_MS)
    }
  }, [initInProgress, initFailedError, displayedPhase, fadingOut])

  // ---- Cleanup ----
  useEffect(
    () => () => {
      if (pumpTimerRef.current) clearTimeout(pumpTimerRef.current)
      if (slowTimerRef.current) clearTimeout(slowTimerRef.current)
      if (fadeTimerRef.current) clearTimeout(fadeTimerRef.current)
    },
    [],
  )

  // ---- Retry handler ----
  async function handleRetry() {
    if (!lastAddVaultArgs) return
    // Reset state machine; new init cycle restarts at phase 0.
    setDisplayedPhase(0)
    setLatestKnownPhase(0)
    setSlowHint(false)
    setFadingOut(false)
    displayedPhaseRef.current = 0
    latestKnownPhaseRef.current = 0
    phaseEnteredAtRef.current = 0
    if (pumpTimerRef.current) {
      clearTimeout(pumpTimerRef.current)
      pumpTimerRef.current = null
    }
    try {
      const entry = await addVault(lastAddVaultArgs.path, lastAddVaultArgs.mode)
      openRoute(entry)
    } catch {
      // Error already piped back into the store as initFailedError; the
      // overlay re-renders failure mode automatically.
    }
  }

  // ---- Visibility / render state ----
  const failed = initFailedError !== null
  const hasPendingDisplay =
    displayedPhase > 0 || latestKnownPhase > displayedPhase
  const visible =
    initInProgress || failed || hasPendingDisplay || fadingOut

  const visiblePhase = clampPhase(displayedPhase)
  const showDots = visiblePhase >= 1 || failed

  // useMemo MUST run on every render to keep hook order stable; the
  // result is only consumed when `visible` is true.
  const subtitleNode = useMemo(() => {
    if (failed && initFailedError) {
      return t(initFailedError.key, initFailedError.vars)
    }
    if (visiblePhase === 0) {
      return t("loading.subtitle")
    }
    return t(PHASE_TITLE_KEYS[visiblePhase as 1 | 2 | 3 | 4 | 5 | 6])
  }, [failed, initFailedError, visiblePhase, t])

  if (!visible) return null

  const titleNode = failed ? t("loading.error.title") : t("loading.title")

  const busAnimation = failed
    ? "codebus-bus-roll 1.8s cubic-bezier(0.45, 0, 0.55, 1) infinite paused"
    : "codebus-bus-roll 1.8s cubic-bezier(0.45, 0, 0.55, 1) infinite"

  return (
    <div
      data-testid="loading-overlay"
      data-tauri-drag-region
      role="status"
      aria-live="polite"
      className="fixed inset-0 z-50 flex select-none items-center justify-center bg-bg/90 backdrop-blur-sm transition-opacity duration-200"
      style={{ opacity: fadingOut ? 0 : 1 }}
    >
      <div
        data-tauri-drag-region
        className="flex flex-col items-center gap-4 px-6"
      >
        {/* The horizontal flip (face right) is baked into the
            codebus-bus-roll keyframes alongside the per-frame transform, so
            the bus rolls forward in the direction it faces with no parent
            wrapper. */}
        <div
          aria-hidden="true"
          className="text-[72px] leading-none"
          style={{ animation: busAnimation }}
        >
          🚌
        </div>
        <h2 className="text-h-row font-semibold tracking-tight">{titleNode}</h2>
        <p className="max-w-[420px] text-center text-meta text-fg-secondary">
          {subtitleNode}
        </p>
        {showDots && (
          <PhaseDots
            total={6}
            current={Math.max(1, visiblePhase)}
            state={failed ? "error" : "running"}
            testId="loading-overlay-phase-dots"
          />
        )}
        {slowHint && !failed && (
          <p
            data-testid="loading-overlay-slow-hint"
            className="max-w-[420px] text-center text-meta text-fg-tertiary"
          >
            {t("loading.slow.hint")}
          </p>
        )}
        {failed && (
          <div className="flex items-center gap-3 pt-2">
            <button
              type="button"
              data-testid="loading-overlay-retry"
              onClick={() => void handleRetry()}
              className="rounded-md border border-warn bg-warn/10 px-3 py-1.5 text-sm font-medium text-fg hover:bg-warn/20"
            >
              {t("loading.error.retry")}
            </button>
            <button
              type="button"
              data-testid="loading-overlay-dismiss"
              onClick={() => clearInitFailure()}
              className="text-meta text-fg-tertiary hover:text-fg-secondary"
            >
              {t("common.dismiss")}
            </button>
          </div>
        )}
      </div>
    </div>
  )
}

function clampPhase(p: number): number {
  if (p < 0) return 0
  if (p > 6) return 6
  return p
}
