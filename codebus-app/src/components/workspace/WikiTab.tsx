import { useEffect, useState } from "react"
import { Folder } from "lucide-react"

import { Button } from "@/components/ui/button"
import { cn } from "@/lib/cn"
import { useWikiStore } from "@/store/wiki"
import { useWatcherEvent } from "@/hooks/useWatcherEvent"
import { useT } from "@/i18n/useT"

import { WatcherStatusBanner } from "./WatcherStatusBanner"
import { WikiPreview } from "./WikiPreview"
import { WikiTree } from "./WikiTree"

interface WikiTabProps {
  vaultPath: string
  /** task 5.3 — forwarded to WikiPreview's `[Quiz me on this]`. */
  onQuizMeOnThis?: (pagePath: string) => void
  /**
   * wiki-page-reader-v1.1 / WP5: forwarded to WikiPreview's edit hint
   * footer. Workspace receives this and pushes the prefilled goal text
   * into the Goals tab's NewGoalModal via pending pattern.
   */
  onRequestNewGoal?: (prefilledText: string) => void
  /**
   * wiki-page-reader-v1.1 / WK-EMPTY: forwarded to the empty-vault hero
   * CTA. Workspace receives this and switches to the Goals tab + opens
   * the NewGoalModal without pre-fill.
   */
  onWikiEmptyCta?: () => void
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
export function WikiTab({
  vaultPath,
  onQuizMeOnThis,
  onRequestNewGoal,
  onWikiEmptyCta,
}: WikiTabProps) {
  const t = useT()
  const pages = useWikiStore((s) => s.pages)
  const currentPath = useWikiStore((s) => s.currentPath)
  const body = useWikiStore((s) => s.body)
  const loadPage = useWikiStore((s) => s.loadPage)
  const listPages = useWikiStore((s) => s.listPages)
  const [treeOpen, setTreeOpen] = useState(true)

  // External edits (Obsidian / VS Code / terminal goal) that touch
  // `<vault>/.codebus/wiki/` SHALL refresh the tree without requiring
  // a manual remount. Spec: `Wiki Tab Subscribes To Watcher Events`.
  useEffect(
    () => useWatcherEvent("wiki-list-changed", () => {
      void listPages(vaultPath)
    }),
    [listPages, vaultPath],
  )

  const hasPages = Object.keys(pages).length > 0
  const currentTitle =
    (currentPath && pages[currentPath]?.title) || currentPath || ""

  if (!hasPages) {
    // WK-EMPTY-1/2/3 design v1.1 spec lock: replace the v1 single-line
    // hint with a hero icon + title + subtitle + amber primary CTA.
    return (
      <div
        data-testid="wiki-tab"
        className="flex h-full w-full items-center justify-center"
      >
        <div
          data-testid="wiki-empty-hero"
          className="flex flex-col items-center gap-4 px-8 text-center"
        >
          <Folder
            aria-hidden="true"
            className="h-14 w-14 text-fg-quaternary"
          />
          <h2 className="text-h-empty font-medium text-fg-primary">
            {t("workspace.wiki.emptyHero.title")}
          </h2>
          <p className="max-w-[420px] text-body text-fg-secondary">
            {t("workspace.wiki.emptyHero.subtitle")}
          </p>
          <Button
            data-testid="wiki-empty-cta"
            variant="primary"
            onClick={() => onWikiEmptyCta?.()}
          >
            {t("workspace.wiki.emptyHero.cta")}
          </Button>
        </div>
      </div>
    )
  }

  return (
    <div
      data-testid="wiki-tab"
      className="flex h-full w-full flex-col"
    >
      <WatcherStatusBanner vaultPath={vaultPath} />
      <div
        data-tauri-drag-region
        className="flex items-center gap-2 border-b border-border px-3 py-2 pr-[160px]"
      >
        <button
          type="button"
          data-testid="wiki-tree-toggle"
          aria-label={t("workspace.wiki.toggleTreeAria")}
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
          className="truncate text-body"
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
            onRequestNewGoal={onRequestNewGoal}
          />
        </div>
      </div>
    </div>
  )
}
