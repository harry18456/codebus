import { useEffect, useState } from "react"
import { Minus, Square, Copy, X } from "lucide-react"

import { cn } from "@/lib/cn"
import { useT } from "@/i18n/useT"

/**
 * Frameless-window control strip. Lives top-right at z-60 so the Settings
 * modal (z-50) does not occlude it — the user may still minimize / restore
 * while a dialog is open. Tauri APIs are dynamically imported so the build
 * stays clean in jsdom tests.
 */
export function WindowControls() {
  const t = useT()
  const [maximized, setMaximized] = useState(false)

  useEffect(() => {
    let cancelled = false
    let unlisten: (() => void) | undefined

    ;(async () => {
      try {
        const mod = await import("@tauri-apps/api/window")
        const win = mod.getCurrentWindow()
        const initial = await win.isMaximized()
        if (!cancelled) setMaximized(initial)
        unlisten = await win.onResized(async () => {
          const isMax = await win.isMaximized()
          if (!cancelled) setMaximized(isMax)
        })
      } catch {
        // Not running under Tauri (jsdom / browser preview) — leave default.
      }
    })()

    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [])

  async function callWindow(method: "minimize" | "toggleMaximize" | "close") {
    try {
      const mod = await import("@tauri-apps/api/window")
      const win = mod.getCurrentWindow()
      await win[method]()
    } catch {
      // No-op outside Tauri.
    }
  }

  return (
    <div
      data-testid="window-controls"
      data-tauri-drag-region={false}
      className="fixed top-0 right-0 z-[60] flex h-11 items-stretch"
    >
      <ControlButton
        label={t("windowControls.minimize")}
        onClick={() => void callWindow("minimize")}
      >
        <Minus className="h-3.5 w-3.5" />
      </ControlButton>
      <ControlButton
        label={
          maximized ? t("windowControls.restore") : t("windowControls.maximize")
        }
        onClick={() => void callWindow("toggleMaximize")}
      >
        {maximized ? (
          <Copy className="h-3 w-3 -scale-x-100" />
        ) : (
          <Square className="h-3 w-3" />
        )}
      </ControlButton>
      <ControlButton
        label={t("windowControls.close")}
        onClick={() => void callWindow("close")}
        danger
      >
        <X className="h-3.5 w-3.5" />
      </ControlButton>
    </div>
  )
}

function ControlButton({
  label,
  danger,
  children,
  onClick,
}: {
  label: string
  danger?: boolean
  children: React.ReactNode
  onClick: () => void
}) {
  return (
    <button
      aria-label={label}
      title={label}
      onClick={onClick}
      className={cn(
        "flex w-[46px] items-center justify-center text-fg-secondary transition-colors",
        "focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-ring",
        danger
          ? "hover:bg-error hover:text-bg"
          : "hover:bg-bg-hover hover:text-fg",
      )}
    >
      {children}
    </button>
  )
}
