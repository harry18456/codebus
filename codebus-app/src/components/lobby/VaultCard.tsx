import { useRef, useState } from "react"
import { MoreVertical } from "lucide-react"

import { formatLastOpened } from "@/lib/time"
import type { VaultEntry } from "@/lib/ipc"
import { cn } from "@/lib/cn"
import { useT } from "@/i18n/useT"

interface VaultCardProps {
  vault: VaultEntry
  onOpen: (vault: VaultEntry) => void
  onRemove: (vault: VaultEntry) => void
  onRevealInFiles: (vault: VaultEntry) => void
}

export function VaultCard({
  vault,
  onOpen,
  onRemove,
  onRevealInFiles,
}: VaultCardProps) {
  const t = useT()
  const [menuOpen, setMenuOpen] = useState(false)
  const [menuPos, setMenuPos] = useState<{ x: number; y: number }>({ x: 0, y: 0 })
  const kebabRef = useRef<HTMLButtonElement | null>(null)

  function openMenuFromKebab(e: React.MouseEvent | React.KeyboardEvent) {
    e.stopPropagation()
    const rect = kebabRef.current?.getBoundingClientRect()
    if (rect) {
      // anchor menu to the kebab button's bottom-left corner
      setMenuPos({ x: rect.left, y: rect.bottom })
    }
    setMenuOpen(true)
  }

  return (
    <div
      data-testid={`vault-card-${vault.path}`}
      role="button"
      tabIndex={0}
      onClick={() => onOpen(vault)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault()
          onOpen(vault)
        }
      }}
      onContextMenu={(e) => {
        e.preventDefault()
        setMenuPos({ x: e.clientX, y: e.clientY })
        setMenuOpen(true)
      }}
      className={cn(
        "group relative flex flex-col gap-1 rounded-lg border border-border bg-bg-raised p-3.5",
        "transition-colors hover:border-border-strong",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent-ring",
        vault.is_missing && "opacity-60",
      )}
    >
      <div className="flex items-baseline justify-between gap-3 pr-7">
        <div className="text-sm font-semibold">{vault.display_name}</div>
        <div className="font-mono text-meta text-fg-tertiary truncate max-w-[60%]">
          {vault.path}
        </div>
      </div>
      <div className="flex items-center gap-2 text-meta text-fg-secondary">
        <span>{t("vaultCard.lastOpened")}</span>
        <span className="font-mono">
          {formatLastOpened(vault.last_opened, t)}
        </span>
        {vault.is_missing && (
          <span
            data-testid="missing-badge"
            className="ml-2 rounded-sm border border-error/40 bg-error/10 px-1.5 py-px font-mono text-micro text-error"
          >
            {t("vaultCard.missingBadge")}
          </span>
        )}
      </div>
      <button
        ref={kebabRef}
        type="button"
        data-testid={`vault-card-kebab-${vault.path}`}
        aria-label={t("vaultCard.menu.openLabel")}
        aria-haspopup="menu"
        aria-expanded={menuOpen}
        onClick={openMenuFromKebab}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault()
            openMenuFromKebab(e)
          }
        }}
        className={cn(
          "absolute right-2 top-2 inline-flex h-7 w-7 items-center justify-center rounded-md",
          "text-fg-tertiary hover:text-fg hover:bg-bg-hover",
          "opacity-0 transition-opacity",
          "group-hover:opacity-100 focus-visible:opacity-100",
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent-ring",
        )}
      >
        <MoreVertical className="h-4 w-4" aria-hidden="true" />
      </button>
      {menuOpen && (
        <div
          role="menu"
          className="fixed z-50 min-w-[180px] rounded-md border border-border bg-bg-raised text-fg shadow-lg"
          style={{ top: menuPos.y, left: menuPos.x }}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            role="menuitem"
            className="block w-full px-3 py-1.5 text-left text-xs hover:bg-bg-hover"
            onClick={() => {
              onRevealInFiles(vault)
              setMenuOpen(false)
            }}
          >
            {t("vaultCard.menu.revealInFiles")}
          </button>
          <button
            role="menuitem"
            className="block w-full px-3 py-1.5 text-left text-xs text-error hover:bg-bg-hover"
            onClick={() => {
              onRemove(vault)
              setMenuOpen(false)
            }}
          >
            {t("vaultCard.menu.remove")}
          </button>
        </div>
      )}
      {menuOpen && (
        <div
          className="fixed inset-0 z-40"
          onClick={() => setMenuOpen(false)}
          aria-hidden="true"
        />
      )}
    </div>
  )
}
