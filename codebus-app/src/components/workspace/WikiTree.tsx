import { useMemo } from "react"

import type { WikiPageMeta } from "@/lib/ipc"
import { cn } from "@/lib/cn"

/**
 * Spec: app-workspace § Wiki Tab with Collapsible File Tree.
 *
 * Group `useWikiStore.pages` by taxonomy folder (concepts / entities /
 * modules / processes / synthesis) and render each row as a clickable
 * button that sets `currentPath` via the parent's `onSelectSlug`
 * callback.
 *
 * Pages whose path does not fall under one of the five taxonomy
 * folders are bucketed under an "Other" group so the tree never
 * silently drops a real page.
 */
const TAXONOMY: readonly string[] = [
  "concepts",
  "entities",
  "modules",
  "processes",
  "synthesis",
] as const

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
  const groups = useMemo(() => groupByTaxonomy(pages), [pages])

  return (
    <nav
      data-testid="wiki-tree"
      className="h-full w-[180px] overflow-auto border-r border-border bg-bg-sunken p-2"
    >
      {[...TAXONOMY, "other"].map((folder) => {
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
    </nav>
  )
}

function groupByTaxonomy(
  pages: Record<string, WikiPageMeta>,
): Record<string, WikiPageMeta[]> {
  const groups: Record<string, WikiPageMeta[]> = {}
  for (const page of Object.values(pages)) {
    const folder = detectFolder(page.path)
    if (!groups[folder]) groups[folder] = []
    groups[folder].push(page)
  }
  for (const list of Object.values(groups)) {
    list.sort((a, b) => a.slug.localeCompare(b.slug))
  }
  return groups
}

function detectFolder(path: string): string {
  const normalized = path.replace(/\\/g, "/")
  for (const folder of TAXONOMY) {
    if (normalized.includes(`/wiki/${folder}/`)) return folder
  }
  return "other"
}
