import { useT } from "@/i18n/useT"

/**
 * Full-window overlay shown while `addVault` is running an init-heavy
 * branch. z-50 so the frameless WindowControls (z-60) stay clickable.
 */
export function LoadingOverlay() {
  const t = useT()
  return (
    <div
      data-testid="loading-overlay"
      data-tauri-drag-region
      role="status"
      aria-live="polite"
      className="fixed inset-0 z-50 flex select-none items-center justify-center bg-bg/90 backdrop-blur-sm"
    >
      <div
        data-tauri-drag-region
        className="flex flex-col items-center gap-4 px-6"
      >
        {/* large glyph, intentionally outside type scale */}
        <div
          className="text-[72px] leading-none"
          aria-hidden="true"
          style={{
            animation:
              "codebus-bus-roll 1.8s cubic-bezier(0.45, 0, 0.55, 1) infinite",
          }}
        >
          🚌
        </div>
        <h2 className="text-h-row font-semibold tracking-tight">
          {t("loading.title")}
        </h2>
        <p className="max-w-[420px] text-center text-meta text-fg-secondary">
          {t("loading.subtitle")}
        </p>
      </div>
    </div>
  )
}
