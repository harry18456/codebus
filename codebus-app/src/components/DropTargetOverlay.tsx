import { useT } from "@/i18n/useT"

/**
 * Full-window drop-target indicator shown while the user is dragging a
 * folder over the Lobby. Sits at z-30 — above main content, below
 * Settings modal (z-50) and WindowControls (z-60). Visual: dark dimmed
 * backdrop + amber dashed inset border + centered hero copy. Matches the
 * pattern used by VS Code / Linear / GitHub Desktop.
 */
export function DropTargetOverlay() {
  const t = useT()
  return (
    <div
      data-testid="drop-target-overlay"
      role="presentation"
      aria-hidden="true"
      className="pointer-events-none fixed inset-0 z-30 flex items-center justify-center bg-bg/85 backdrop-blur-[2px]"
    >
      <div className="absolute inset-6 rounded-xl border-2 border-dashed border-accent" />
      <div className="relative flex flex-col items-center gap-3 px-6">
        <div className="text-[64px] leading-none" aria-hidden="true">
          🚌
        </div>
        <h2 className="text-[20px] font-semibold tracking-tight text-accent">
          {t("dropTarget.title")}
        </h2>
        <p className="max-w-[420px] text-center text-[12px] text-fg-secondary">
          {t("dropTarget.subtitle")}
        </p>
      </div>
    </div>
  )
}
