import { useMemo } from "react"
import { BookOpen } from "lucide-react"

import type { WikiPageMeta } from "@/lib/ipc"
import { cn } from "@/lib/cn"
import { useT } from "@/i18n/useT"

/**
 * Spec: app-workspace § Wiki Tab with Collapsible File Tree +
 * § Wiki Tree Travel Log Footer Slot.
 *
 * Layout (top → bottom):
 *   1. Wiki Index entry (slug `index`), pinned at the top.
 *   2. Five taxonomy buckets (concepts / entities / modules /
 *      processes / synthesis) rendered in that order, each only
 *      when it has at least one entry.
 *   3. Travel log footer slot — the system `log.md` entry, separated
 *      from the buckets above by a hairline border + 18px gap.
 *
 * The previous catch-all "Other" bucket is no longer rendered. Pages
 * whose path does not fall under one of the five taxonomy folders
 * remain reachable through the page index but do not surface a bucket
 * header. `index.md` and `log.md` are handled by the top entry + the
 * footer slot respectively (per WK2 / WP-tree-footer design v1.1).
 */
const TAXONOMY: readonly string[] = [
  "concepts",
  "entities",
  "modules",
  "processes",
  "synthesis",
] as const

const INDEX_SLUG = "index"
const LOG_SLUG = "log"

interface WikiTreeProps {
  pages: Record<string, WikiPageMeta>
  currentSlug: string | null
  onSelectSlug: (slug: string) => void
}

export function WikiTree({
  pages,
  currentSlug,
  onSelectSlug,
}: WikiTreeProps) {
  const t = useT()
  const groups = useMemo(() => groupByTaxonomy(pages), [pages])
  const indexPage = pages[INDEX_SLUG]
  const logPage = pages[LOG_SLUG]

  return (
    <nav
      data-testid="wiki-tree"
      className="h-full w-[180px] overflow-auto border-r border-border bg-bg-sunken p-2"
    >
      {indexPage && (
        <div data-testid="wiki-tree-index" className="mb-2">
          <ul>
            <li>
              <button
                type="button"
                data-testid={`wiki-tree-row-${indexPage.slug}`}
                onClick={() => onSelectSlug(indexPage.slug)}
                className={cn(
                  "w-full rounded-sm px-1 py-0.5 text-left text-meta",
                  indexPage.slug === currentSlug
                    ? "bg-accent/20 text-accent"
                    : "text-fg-secondary hover:text-fg",
                  "focus:outline-none focus:ring-2 focus:ring-accent-ring",
                )}
              >
                {indexPage.title || indexPage.slug}
              </button>
            </li>
          </ul>
        </div>
      )}
      {TAXONOMY.map((folder) => {
        const entries = groups[folder]
        if (!entries || entries.length === 0) return null
        return (
          <div key={folder} className="mb-2">
            <h4
              data-testid={`wiki-tree-group-${folder}`}
              className="px-1 text-micro font-semibold uppercase tracking-wide text-fg-tertiary"
            >
              {folder}
            </h4>
            <ul>
              {entries.map((page) => (
                <li key={page.slug}>
                  <button
                    type="button"
                    data-testid={`wiki-tree-row-${page.slug}`}
                    onClick={() => onSelectSlug(page.slug)}
                    className={cn(
                      "w-full rounded-sm px-1 py-0.5 text-left text-meta",
                      page.slug === currentSlug
                        ? "bg-accent/20 text-accent"
                        : "text-fg-secondary hover:text-fg",
                      "focus:outline-none focus:ring-2 focus:ring-accent-ring",
                    )}
                  >
                    {page.title || page.slug}
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )
      })}
      {logPage && (
        <div
          data-testid="wiki-tree-footer-slot"
          className="mt-[18px] border-t border-border pt-2"
        >
          <button
            type="button"
            data-testid={`wiki-tree-row-${logPage.slug}`}
            onClick={() => onSelectSlug(logPage.slug)}
            className={cn(
              "flex w-full items-center gap-2 rounded-sm px-1 py-0.5 text-left text-meta",
              logPage.slug === currentSlug
                ? "bg-accent/20 text-accent"
                : "text-fg-tertiary hover:text-fg",
              "focus:outline-none focus:ring-2 focus:ring-accent-ring",
            )}
          >
            <BookOpen aria-hidden="true" className="h-3.5 w-3.5" />
            <span>{t("workspace.wiki.travelLogLabel")}</span>
          </button>
        </div>
      )}
    </nav>
  )
}

function groupByTaxonomy(
  pages: Record<string, WikiPageMeta>,
): Record<string, WikiPageMeta[]> {
  const groups: Record<string, WikiPageMeta[]> = {}
  for (const page of Object.values(pages)) {
    if (page.slug === INDEX_SLUG || page.slug === LOG_SLUG) continue
    const folder = detectFolder(page.path)
    if (folder === null) continue
    if (!groups[folder]) groups[folder] = []
    groups[folder].push(page)
  }
  for (const list of Object.values(groups)) {
    list.sort((a, b) => a.slug.localeCompare(b.slug))
  }
  return groups
}

function detectFolder(path: string): string | null {
  const normalized = path.replace(/\\/g, "/")
  for (const folder of TAXONOMY) {
    if (normalized.includes(`/wiki/${folder}/`)) return folder
  }
  return null
}
