import { Settings } from "lucide-react"

import { useT } from "@/i18n/useT"

interface BottomStripProps {
  version: string
  onOpenSettings: () => void
}

export function BottomStrip({ version, onOpenSettings }: BottomStripProps) {
  const t = useT()
  return (
    <footer
      data-testid="bottom-strip"
      className="flex h-8 w-full items-center justify-between border-t border-border bg-bg-sunken px-4"
    >
      <button
        aria-label={t("bottomStrip.settings")}
        data-testid="settings-gear"
        onClick={onOpenSettings}
        className="flex items-center gap-1.5 text-xs text-fg-secondary hover:text-fg focus:outline-none focus:ring-2 focus:ring-accent-ring rounded-sm"
      >
        <Settings className="h-3.5 w-3.5" />
        {t("bottomStrip.settings")}
      </button>
      <span
        data-testid="version-label"
        className="font-mono text-meta text-fg-tertiary"
      >
        {version}
      </span>
    </footer>
  )
}
