import * as DialogPrimitive from "@radix-ui/react-dialog"

import { useChatStore } from "@/store/chat"
import { useGoalsStore } from "@/store/goals"
import { useT } from "@/i18n/useT"

import { ChatInput } from "./ChatInput"
import { ChatNewChatButton } from "./ChatNewChatButton"
import { ChatTokenDisplay } from "./ChatTokenDisplay"
import { ChatTranscript } from "./ChatTranscript"
import { ChatUndoToast } from "./ChatUndoToast"

/**
 * Bottom-right anchored chat widget shell with three rendering modes
 * (per openspec/specs/app-workspace "Chat Widget Layout and Two-State
 * Toggle" requirement):
 *
 *  - **bubble**   — 44×44 circular `💬` bubble pinned to viewport
 *    bottom-right (16px from each edge, above the 32px `BottomStrip`).
 *    Shows a small red `chat-widget-promote-badge` while a pending
 *    PromoteSuggestion exists, plus a 7px amber active-goal pulse dot
 *    when `useGoalsStore.activeRun` is non-null.
 *  - **floating** — 360×460 fixed-size panel anchored to the same
 *    bottom-right point. Header carries `⤢` expand-to-modal + `▿`
 *    minimize buttons. Esc is intentionally a no-op (per design
 *    "Esc 行為分層" decision — floating mode is "sticky").
 *  - **modal**    — 640-wide × max 480 tall centered modal rendered via
 *    the project's radix Dialog primitive (focus trap + aria-modal +
 *    restore focus on close are all provided by radix). Header has
 *    `⤡` dock-to-floating + `✕` close. Esc / backdrop click trigger
 *    `closeModalToReturnMode()`; the `✕` button explicitly bypasses
 *    `modalReturnMode` and lands in bubble per the spec scenario
 *    "Modal close button always returns to bubble".
 *
 * Mode transitions are driven entirely by the `useChatStore` reducer
 * (see `openFloating / minimizeToBubble / openModal / dockToFloating /
 * closeModalToReturnMode / closeModalToBubble`). The renderer is
 * stateless and never holds local mode state.
 *
 * `vaultPath / onPromoteSuccess / onWikiLinkClick` are wired by
 * `Workspace.tsx`; all three are optional so the widget can render in
 * isolated unit tests without vault wiring.
 */

// Layout constants — see AUDIT R7-modes lock 2026-05-26.
// BottomStrip is 32px tall; widget hugs the bottom with an additional
// 16px gap so the version label / settings gear stay reachable.
const BOTTOM_STRIP_PX = 32
const EDGE_GAP_PX = 16
const EDGE_OFFSET_PX = BOTTOM_STRIP_PX + EDGE_GAP_PX
const EDGE_OFFSET_RIGHT_PX = EDGE_GAP_PX

// Floating panel is fixed-size per AUDIT R7-modes (resize handle was
// removed by the REMOVED "Chat Widget Resize Affordance" requirement).
const FLOATING_WIDTH_PX = 360
const FLOATING_HEIGHT_PX = 460

// Modal sizing per spec "Modal mode renders centered with backdrop".
// Top offset 60px from viewport top — visual weight sits above center.
const MODAL_WIDTH_PX = 640
const MODAL_MAX_HEIGHT_PX = 480
const MODAL_TOP_PX = 60

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
  const mode = useChatStore((s) => s.mode)
  const promoteSuggestion = useChatStore((s) => s.promoteSuggestion)
  const openFloating = useChatStore((s) => s.openFloating)
  // Selector form yields a boolean so the bubble only re-renders on
  // null↔non-null transitions, not every stream event. Coerce
  // `undefined` (store wiped in some test races) to false so the dot
  // degrades to invisible rather than crashing.
  const hasActiveGoal = useGoalsStore((s) => s.activeRun != null)
  const t = useT()

  if (mode === "bubble") {
    return (
      <button
        type="button"
        data-testid="chat-widget"
        data-state="bubble"
        aria-label={t(
          hasActiveGoal
            ? "chat.widget.aria.openChatWithActiveGoalRunning"
            : "chat.widget.aria.openChat",
        )}
        onClick={openFloating}
        className="fixed z-50 flex h-11 w-11 items-center justify-center rounded-full border border-border bg-bg-raised text-2xl text-fg shadow-lg transition-colors hover:bg-bg-hover focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-ring"
        style={{
          bottom: `${EDGE_OFFSET_PX}px`,
          right: `${EDGE_OFFSET_RIGHT_PX}px`,
        }}
      >
        <span aria-hidden="true">💬</span>
        {/*
          Active-goal pulse dot. Always mounted on the bubble so the
          200ms opacity transition can play in both directions — the
          design rejected `unmount-on-clear` because that loses the
          fade-out animation. Positioned further into the corner than
          `chat-widget-promote-badge` so both indicators can coexist
          when a promote suggestion lands while a goal is running.
          `motion-reduce` variant drops the transition for users who
          request reduced motion.
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

  if (mode === "floating") {
    return (
      <FloatingPanel
        vaultPath={vaultPath}
        onPromoteSuccess={onPromoteSuccess}
        onWikiLinkClick={onWikiLinkClick}
      />
    )
  }

  // mode === "modal"
  return (
    <ModalDialog
      vaultPath={vaultPath}
      onPromoteSuccess={onPromoteSuccess}
      onWikiLinkClick={onWikiLinkClick}
    />
  )
}

interface PanelProps {
  vaultPath?: string
  onPromoteSuccess?: (runId: string) => void
  onWikiLinkClick?: (slug: string) => void
}

function FloatingPanel({
  vaultPath,
  onPromoteSuccess,
  onWikiLinkClick,
}: PanelProps) {
  const t = useT()
  const minimizeToBubble = useChatStore((s) => s.minimizeToBubble)
  const openModal = useChatStore((s) => s.openModal)
  return (
    <div
      data-testid="chat-widget"
      data-state="floating"
      role="region"
      aria-label={t("chat.widget.aria.floating.title")}
      className="fixed z-50 flex flex-col overflow-hidden rounded-lg border border-border bg-bg-raised text-fg shadow-xl"
      style={{
        bottom: `${EDGE_OFFSET_PX}px`,
        right: `${EDGE_OFFSET_RIGHT_PX}px`,
        width: `${FLOATING_WIDTH_PX}px`,
        height: `${FLOATING_HEIGHT_PX}px`,
      }}
    >
      <div
        data-testid="chat-widget-header-slot"
        className="flex flex-none items-center gap-2 border-b border-border bg-bg-sunken px-3 py-1.5"
      >
        <ChatNewChatButton />
        <ChatTokenDisplay />
        <button
          type="button"
          data-testid="chat-widget-expand-to-modal"
          aria-label={t("chat.widget.aria.floating.expandToModal")}
          onClick={openModal}
          className="ml-auto flex h-6 w-6 items-center justify-center rounded-md text-fg-secondary hover:bg-bg-hover hover:text-fg focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-ring"
        >
          <span aria-hidden="true" className="text-base leading-none">⤢</span>
        </button>
        <button
          type="button"
          data-testid="chat-widget-minimize"
          aria-label={t("chat.widget.aria.floating.minimize")}
          onClick={minimizeToBubble}
          className="flex h-6 w-6 items-center justify-center rounded-md text-fg-secondary hover:bg-bg-hover hover:text-fg focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-ring"
        >
          <span aria-hidden="true" className="text-base leading-none">−</span>
        </button>
      </div>
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

function ModalDialog({
  vaultPath,
  onPromoteSuccess,
  onWikiLinkClick,
}: PanelProps) {
  const t = useT()
  const dockToFloating = useChatStore((s) => s.dockToFloating)
  const closeModalToBubble = useChatStore((s) => s.closeModalToBubble)
  const closeModalToReturnMode = useChatStore((s) => s.closeModalToReturnMode)
  return (
    <DialogPrimitive.Root
      open
      // Radix fires onOpenChange(false) for Esc / backdrop / built-in
      // close requests. Our explicit ✕ button bypasses this by calling
      // closeModalToBubble() directly (which flips `open` to false but
      // does not fire onOpenChange because the prop is externally
      // controlled), so onOpenChange is only the Esc / backdrop path.
      onOpenChange={(open) => {
        if (!open) closeModalToReturnMode()
      }}
    >
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay
          className="fixed inset-0 z-50 bg-black/55 backdrop-blur-[2px] data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 motion-reduce:animate-none motion-reduce:transition-none"
        />
        <DialogPrimitive.Content
          data-testid="chat-widget"
          data-state="modal"
          aria-modal="true"
          className="fixed left-1/2 z-50 flex -translate-x-1/2 flex-col overflow-hidden rounded-xl border border-border bg-bg-raised text-fg shadow-2xl motion-reduce:animate-none motion-reduce:transition-none"
          style={{
            top: `${MODAL_TOP_PX}px`,
            width: `${MODAL_WIDTH_PX}px`,
            maxWidth: "90vw",
            maxHeight: `${MODAL_MAX_HEIGHT_PX}px`,
          }}
        >
          <div
            data-testid="chat-widget-header-slot"
            className="flex flex-none items-center gap-2 border-b border-border bg-bg-sunken px-3 py-1.5"
          >
            <DialogPrimitive.Title className="flex items-center gap-2 text-xs font-semibold">
              <span aria-hidden="true">💬</span>
              <span>{t("chat.widget.aria.modal.title")}</span>
            </DialogPrimitive.Title>
            <ChatNewChatButton />
            <ChatTokenDisplay />
            <button
              type="button"
              data-testid="chat-widget-dock-to-floating"
              aria-label={t("chat.widget.aria.modal.dockToFloating")}
              onClick={dockToFloating}
              className="ml-auto flex h-6 w-6 items-center justify-center rounded-md text-fg-secondary hover:bg-bg-hover hover:text-fg focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-ring"
            >
              <span aria-hidden="true" className="text-base leading-none">⤡</span>
            </button>
            <button
              type="button"
              data-testid="chat-widget-modal-close"
              aria-label={t("chat.widget.aria.modal.close")}
              onClick={closeModalToBubble}
              className="flex h-6 w-6 items-center justify-center rounded-md text-fg-secondary hover:bg-bg-hover hover:text-fg focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-ring"
            >
              <span aria-hidden="true" className="text-base leading-none">✕</span>
            </button>
          </div>
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
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  )
}
