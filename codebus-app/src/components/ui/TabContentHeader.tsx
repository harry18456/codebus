import type { ReactNode } from "react"

export interface TabContentHeaderProps {
  title: string
  subtitle?: string
  cta?: ReactNode
  shortcutChipText?: string
  /**
   * Optional slot rendered between the title block and the cta. Used by
   * the Quiz wizard to surface step dots / "Step X / N" labels while a
   * wizard flow is active (see app-workspace § Quiz Tab Wizard Content
   * Header And Layout). Existing callers that omit this prop render
   * exactly as before.
   */
  stepIndicator?: ReactNode
  testId?: string
}

export function TabContentHeader({
  title,
  subtitle,
  cta,
  shortcutChipText,
  stepIndicator,
  testId,
}: TabContentHeaderProps) {
  const showChip = Boolean(cta) && Boolean(shortcutChipText)

  return (
    <div
      data-tauri-drag-region
      data-testid={testId}
      className="flex items-center justify-between border-b border-border p-3 pr-[160px]"
    >
      <div className="flex flex-col gap-0.5">
        <h1 className="text-h-row font-medium text-fg-primary">{title}</h1>
        {subtitle && (
          <p className="text-meta text-fg-secondary">{subtitle}</p>
        )}
      </div>
      {stepIndicator && (
        <div data-tch-step-indicator className="flex items-center gap-2">
          {stepIndicator}
        </div>
      )}
      {cta && (
        <div className="flex items-center gap-2">
          <span data-tch-cta>{cta}</span>
          {showChip && (
            <kbd
              data-tch-chip
              aria-hidden="true"
              className="rounded-sm border border-border bg-bg-raised px-1 py-px font-mono text-micro text-fg-tertiary"
            >
              {shortcutChipText}
            </kbd>
          )}
        </div>
      )}
    </div>
  )
}
