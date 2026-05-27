import { useEffect, useRef, type CSSProperties, type PointerEvent as ReactPointerEvent } from "react"
// CSSProperties retained for the small inline-style fragments that still
// need to carry computed rem values (bottom offset + width/height).

import { useChatStore } from "@/store/chat"
import { useGoalsStore } from "@/store/goals"
import { useT } from "@/i18n/useT"

import { ChatInput } from "./ChatInput"
import { ChatNewChatButton } from "./ChatNewChatButton"
import { ChatTokenDisplay } from "./ChatTokenDisplay"
import { ChatTranscript } from "./ChatTranscript"
import { ChatUndoToast } from "./ChatUndoToast"

/**
 * Bottom-right pinned chat widget shell. Implements the "Widget Layout —
 * Bottom-right Corner Pinned, Two States" section of the change design:
 *
 *  - Collapsed: 3rem × 3rem circular `💬` bubble pinned to viewport
 *    bottom-right (16px from each edge). Renders a small red-dot badge while
 *    a `PromoteSuggestion` is pending so the user can see something needs
 *    attention without opening the panel.
 *  - Expanded: panel sized to `useChatStore.{width,height}` (rem units),
 *    pinned to the same bottom-right anchor. The top-left corner is the
 *    *only* resize handle — the other three corners stay locked to the
 *    viewport edges so the panel can never slide out of view.
 *  - Resize drag clamps to `[18, 40]rem` × `[24, 60]rem` AND to
 *    `50% viewport width × 80% viewport height` so the panel always fits.
 *  - A `window.resize` listener re-applies the viewport clamp so shrinking
 *    the OS window auto-shrinks the panel (spec scenario "Viewport shrink
 *    auto-clamps widget").
 *
 * Header / transcript / input / token display / undo toast are owned by
 * sibling tasks; this file only owns the shell, the bubble↔panel toggle,
 * the resize handle, the badge, and the auto-clamp effect.
 */
const MIN_WIDTH_REM = 18
const MAX_WIDTH_REM = 40
const MIN_HEIGHT_REM = 24
const MAX_HEIGHT_REM = 60
const VIEWPORT_WIDTH_RATIO = 0.5
const VIEWPORT_HEIGHT_RATIO = 0.8
const REM_PX = 16
// Workspace mounts a 32px `BottomStrip` (`h-8`) at the viewport bottom
// for the version label + settings gear. Push the widget up so the
// bubble and the expanded panel never sit on top of those controls.
const BOTTOM_STRIP_PX = 32
const EDGE_GAP_PX = 16
const EDGE_OFFSET_PX = BOTTOM_STRIP_PX + EDGE_GAP_PX
const EDGE_OFFSET_RIGHT_PX = EDGE_GAP_PX

function pxToRem(px: number): number {
  return px / REM_PX
}

function clampWidth(widthRem: number): number {
  const viewportCapRem = pxToRem(window.innerWidth) * VIEWPORT_WIDTH_RATIO
  const upper = Math.min(MAX_WIDTH_REM, viewportCapRem)
  return Math.max(MIN_WIDTH_REM, Math.min(widthRem, upper))
}

function clampHeight(heightRem: number): number {
  const viewportCapRem = pxToRem(window.innerHeight) * VIEWPORT_HEIGHT_RATIO
  const upper = Math.min(MAX_HEIGHT_REM, viewportCapRem)
  return Math.max(MIN_HEIGHT_REM, Math.min(heightRem, upper))
}

/**
 * Props wired by `Workspace`:
 *
 * - `vaultPath`: forwarded into `ChatTranscript` (for `acceptPromoteSuggestion`
 *   + onboarding-hint key) and `ChatInput` (for `spawnTurn`). All chat IPC is
 *   vault-scoped so the path must come from the Workspace that owns the
 *   currently-open vault.
 * - `onPromoteSuccess`: called after the inline `[Promote to goal]` pill
 *   resolves with a new run id; `Workspace` uses this to flip the active tab
 *   to Goals + select the freshly-spawned run.
 * - `onWikiLinkClick`: called when an assistant message renders a
 *   `wiki/...md` link and the user clicks it; `Workspace` switches to the
 *   Wiki tab + loads the page.
 *
 * All three are optional so `ChatWidget` can still render in isolated tests
 * (e.g. the existing ChatWidget unit tests don't need vault wiring).
 */
export interface ChatWidgetProps {
  vaultPath?: string
  onPromoteSuccess?: (runId: string) => void
  onWikiLinkClick?: (slug: string) => void
}

export function ChatWidget({
  vaultPath,
  onPromoteSuccess,
  onWikiLinkClick,
}: ChatWidgetProps = {}) {
  const t = useT()
  const expanded = useChatStore((s) => s.expanded)
  const width = useChatStore((s) => s.width)
  const height = useChatStore((s) => s.height)
  const promoteSuggestion = useChatStore((s) => s.promoteSuggestion)
  const toggleExpanded = useChatStore((s) => s.toggleExpanded)
  const setSize = useChatStore((s) => s.setSize)
  // Surface the goal-running indicator on the collapsed bubble so the user
  // can tell from the bottom-right corner that something is in flight even
  // without switching to the Goals tab. Selector form yields a boolean so
  // the bubble only re-renders on null↔non-null transitions, not every
  // stream event. Coerce `undefined` (store not initialised in some tests)
  // to false so the dot degrades to invisible rather than crashing.
  const hasActiveGoal = useGoalsStore((s) => s.activeRun != null)

  // Auto-clamp on viewport resize. Reads fresh width/height from the store
  // each tick so we never compare against stale closure values when several
  // resize events fire in quick succession.
  useEffect(() => {
    function onResize() {
      const { width: curW, height: curH } = useChatStore.getState()
      const nextW = clampWidth(curW)
      const nextH = clampHeight(curH)
      if (nextW !== curW || nextH !== curH) {
        setSize(nextW, nextH)
      }
    }
    window.addEventListener("resize", onResize)
    return () => window.removeEventListener("resize", onResize)
  }, [setSize])

  if (!expanded) {
    return (
      <button
        type="button"
        data-testid="chat-widget"
        data-state="collapsed"
        aria-label={t(
          hasActiveGoal
            ? "chat.widget.aria.openChatWithActiveGoalRunning"
            : "chat.widget.aria.openChat",
        )}
        onClick={toggleExpanded}
        className="fixed z-50 flex h-12 w-12 items-center justify-center rounded-full border border-border bg-bg-raised text-2xl text-fg shadow-lg transition-colors hover:bg-bg-hover focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-ring"
        style={{
          bottom: `${EDGE_OFFSET_PX}px`,
          right: `${EDGE_OFFSET_RIGHT_PX}px`,
        }}
      >
        <span aria-hidden="true">💬</span>
        {/*
          Active-goal pulse dot. Always mounted on the collapsed bubble so
          the 200ms opacity transition can play in both directions — the
          design rejected `unmount-on-clear` because that loses the
          fade-out animation. Positioned further into the corner than
          `chat-widget-promote-badge` (right-1 top-1) so both indicators
          can coexist when a promote suggestion lands while a goal is
          running, per the spec scenario "Pulse dot and promote badge
          render simultaneously without overlap". `motion-reduce` variant
          drops the transition for users who request reduced motion.
        */}
        <span
          data-testid="chat-widget-active-goal-pulse"
          aria-hidden="true"
          className={`absolute right-0.5 top-0.5 h-[7px] w-[7px] rounded-full bg-accent transition-opacity duration-200 motion-reduce:transition-none ${
            hasActiveGoal ? "opacity-100" : "opacity-0"
          }`}
        />
        {promoteSuggestion ? (
          <span
            data-testid="chat-widget-promote-badge"
            aria-hidden="true"
            className="absolute right-1 top-1 h-2.5 w-2.5 rounded-full border-2 border-bg-raised bg-error"
          />
        ) : null}
      </button>
    )
  }

  return (
    <ExpandedPanel
      width={width}
      height={height}
      setSize={setSize}
      vaultPath={vaultPath}
      onPromoteSuccess={onPromoteSuccess}
      onWikiLinkClick={onWikiLinkClick}
    />
  )
}

interface ExpandedPanelProps {
  width: number
  height: number
  setSize: (width: number, height: number) => void
  vaultPath?: string
  onPromoteSuccess?: (runId: string) => void
  onWikiLinkClick?: (slug: string) => void
}

function ExpandedPanel({
  width,
  height,
  setSize,
  vaultPath,
  onPromoteSuccess,
  onWikiLinkClick,
}: ExpandedPanelProps) {
  const t = useT()
  const toggleExpanded = useChatStore((s) => s.toggleExpanded)

  // PointerDown captures starting client coords + the widget's starting
  // (width, height); subsequent pointermove ticks compute new sizes by
  // *subtracting* delta (top-left handle: drag up/left → grow), then clamp.
  // We commit to the store on every pointermove so the resized panel stays
  // in sync visually; pointerup just releases capture.
  const dragRef = useRef<{
    startX: number
    startY: number
    startWidth: number
    startHeight: number
    pointerId: number
  } | null>(null)

  function onPointerDown(e: ReactPointerEvent<HTMLDivElement>) {
    e.preventDefault()
    // JSDOM does not implement Pointer Capture; guard so unit tests can
    // still drive the handle without hitting `setPointerCapture is not a
    // function`. Real browsers do support it and we want capture there so
    // the move/up events keep flowing even when the cursor leaves the
    // 12×12 handle hitbox during a fast drag.
    if (typeof e.currentTarget.setPointerCapture === "function") {
      try {
        e.currentTarget.setPointerCapture(e.pointerId)
      } catch {
        // No-op: capture is best-effort polish, never load-bearing.
      }
    }
    dragRef.current = {
      startX: e.clientX,
      startY: e.clientY,
      startWidth: width,
      startHeight: height,
      pointerId: e.pointerId,
    }
  }

  function onPointerMove(e: ReactPointerEvent<HTMLDivElement>) {
    const drag = dragRef.current
    if (!drag || drag.pointerId !== e.pointerId) return
    const deltaXrem = pxToRem(e.clientX - drag.startX)
    const deltaYrem = pxToRem(e.clientY - drag.startY)
    // Top-left handle: dragging left (-deltaX) grows width; dragging up
    // (-deltaY) grows height. Subtraction gives the intuitive sign.
    const rawW = drag.startWidth - deltaXrem
    const rawH = drag.startHeight - deltaYrem
    setSize(clampWidth(rawW), clampHeight(rawH))
  }

  function onPointerUp(e: ReactPointerEvent<HTMLDivElement>) {
    const drag = dragRef.current
    if (!drag) return
    if (
      typeof e.currentTarget.hasPointerCapture === "function" &&
      typeof e.currentTarget.releasePointerCapture === "function" &&
      e.currentTarget.hasPointerCapture(e.pointerId)
    ) {
      e.currentTarget.releasePointerCapture(e.pointerId)
    }
    dragRef.current = null
  }

  const panelStyle: CSSProperties = {
    bottom: `${EDGE_OFFSET_PX}px`,
    right: `${EDGE_OFFSET_RIGHT_PX}px`,
    width: `${width}rem`,
    height: `${height}rem`,
  }

  return (
    <div
      data-testid="chat-widget"
      data-state="expanded"
      className="fixed z-50 flex flex-col overflow-hidden rounded-lg border border-border bg-bg-raised text-fg shadow-xl"
      style={panelStyle}
    >
      <div
        data-testid="chat-widget-resize-handle"
        role="separator"
        aria-label={t("chat.widget.aria.resizeChat")}
        title={t("chat.widget.title.dragToResize")}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
        className="absolute left-0 top-0 z-10 h-4 w-4 cursor-nwse-resize text-fg-tertiary hover:text-fg"
        style={{ touchAction: "none" }}
      >
        {/* Subtle visual affordance — two diagonal lines forming a corner
            arrow so the user can find the resize grip without inspecting
            the cursor. Rendered as an SVG so it scales with font-size. */}
        <svg
          aria-hidden="true"
          viewBox="0 0 16 16"
          className="h-full w-full"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
        >
          <path d="M3 7 L7 3" />
          <path d="M3 12 L12 3" />
        </svg>
      </div>
      <div
        data-testid="chat-widget-header-slot"
        className="flex flex-none items-center gap-2 border-b border-border bg-bg-sunken px-3 py-1.5"
      >
        <ChatNewChatButton />
        <ChatTokenDisplay />
        <button
          type="button"
          data-testid="chat-widget-minimize"
          aria-label={t("chat.widget.aria.minimizeChat")}
          title={t("chat.widget.title.minimizeShortcut")}
          onClick={toggleExpanded}
          className="ml-1 flex h-6 w-6 items-center justify-center rounded-md text-fg-secondary hover:bg-bg-hover hover:text-fg focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-ring"
        >
          <span aria-hidden="true" className="text-base leading-none">−</span>
        </button>
      </div>
      {/* Undo banner pushes the body slot down by ~32px when active — it
          renders in the layout flow (NOT absolute overlay) so it can
          never occlude the ChatInput at the bottom. Self-hides once the
          store's undo buffer clears. */}
      <ChatUndoToast />
      <div
        data-testid="chat-widget-body-slot"
        className="flex-1 min-h-0 overflow-auto bg-bg-raised"
      >
        <ChatTranscript
          vaultPath={vaultPath}
          onWikiLinkClick={onWikiLinkClick}
          onPromoteSuccess={onPromoteSuccess}
        />
      </div>
      <div
        data-testid="chat-widget-footer-slot"
        className="flex-none border-t border-border bg-bg-sunken"
      >
        {vaultPath ? <ChatInput vaultPath={vaultPath} /> : null}
      </div>
    </div>
  )
}
