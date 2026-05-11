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
      role="status"
      aria-live="polite"
      className="fixed inset-0 z-50 flex items-center justify-center bg-bg/90 backdrop-blur-sm"
    >
      <div className="flex flex-col items-center gap-4 px-6">
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
        <h2 className="text-[18px] font-semibold tracking-tight">
          {t("loading.title")}
        </h2>
        <p className="max-w-[420px] text-center text-[12px] text-fg-secondary">
          {t("loading.subtitle")}
        </p>
      </div>
    </div>
  )
}
