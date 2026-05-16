import { useState } from "react"
import { Folder } from "lucide-react"

import { cn } from "@/lib/cn"
import { useWikiStore } from "@/store/wiki"

import { WikiPreview } from "./WikiPreview"
import { WikiTree } from "./WikiTree"

interface WikiTabProps {
  vaultPath: string
  /** task 5.3 — forwarded to WikiPreview's `[Quiz me on this]`. */
  onQuizMeOnThis?: (pagePath: string) => void
}

/**
 * Spec: app-workspace § Wiki Tab with Collapsible File Tree.
 *
 * - Top bar with a folder-icon toggle button + the currently-selected
 *   page title.
 * - Left-side collapsible `Pages` tree (collapsed by default).
 * - Right-side Milkdown read-only preview.
 * - When the vault has zero wiki pages, render the centered hint
 *   `No wiki pages yet — run a goal to start documenting`.
 */
export function WikiTab({ vaultPath, onQuizMeOnThis }: WikiTabProps) {
  const pages = useWikiStore((s) => s.pages)
  const currentPath = useWikiStore((s) => s.currentPath)
  const body = useWikiStore((s) => s.body)
  const loadPage = useWikiStore((s) => s.loadPage)
  const [treeOpen, setTreeOpen] = useState(true)

  const hasPages = Object.keys(pages).length > 0
  const currentTitle =
    (currentPath && pages[currentPath]?.title) || currentPath || ""

  if (!hasPages) {
    return (
      <div
        data-testid="wiki-tab"
        className="flex h-full w-full items-center justify-center text-center"
      >
        <p
          data-testid="wiki-empty"
          className="text-[13px] text-fg-secondary"
        >
          No wiki pages yet — run a goal to start documenting
        </p>
      </div>
    )
  }

  return (
    <div
      data-testid="wiki-tab"
      className="flex h-full w-full flex-col"
    >
      <div
        data-tauri-drag-region
        className="flex items-center gap-2 border-b border-border px-3 py-2 pr-[160px]"
      >
        <button
          type="button"
          data-testid="wiki-tree-toggle"
          aria-label="Toggle Pages tree"
          aria-pressed={treeOpen}
          onClick={() => setTreeOpen((v) => !v)}
          className={cn(
            "inline-flex h-6 w-6 items-center justify-center rounded-sm",
            "hover:bg-bg-hover focus:outline-none focus:ring-2 focus:ring-accent-ring",
            treeOpen && "bg-bg-hover",
          )}
        >
          <Folder className="h-4 w-4" />
        </button>
        <span
          data-testid="wiki-current-title"
          className="truncate text-[13px]"
        >
          {currentTitle}
        </span>
      </div>
      <div className="flex flex-1 overflow-hidden">
        {treeOpen && (
          <WikiTree
            pages={pages}
            currentSlug={currentPath}
            onSelectSlug={(slug) => void loadPage(vaultPath, slug)}
          />
        )}
        <div className="flex-1 overflow-auto">
          <WikiPreview
            vaultPath={vaultPath}
            body={body}
            onQuizMeOnThis={onQuizMeOnThis}
          />
        </div>
      </div>
    </div>
  )
}
