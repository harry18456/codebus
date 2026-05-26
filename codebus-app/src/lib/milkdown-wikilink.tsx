import type { WikiPageMeta } from "@/lib/ipc"
import { cn } from "@/lib/cn"
import { useT } from "@/i18n/useT"

/**
 * v1 wikilink renderer.
 *
 * Spec: app-workspace § Wikilink Resolution and Click Behavior.
 *
 * Resolution is fully client-side against `useWikiStore.pages`; the
 * component SHALL NOT issue an IPC call when clicked (resolvable
 * clicks route through `useWikiStore.loadPage`, which already has
 * its own cache).
 *
 * Two rendered states:
 * - Resolvable (slug exists in `pages`): clickable colored anchor;
 *   click invokes `onResolve(slug)`.
 * - Unresolvable: dimmed span with `Page not found` tooltip; click
 *   is a no-op.
 *
 * Design § Risks documents that v1 takes the "react-markdown + regex
 * renderer" fallback path instead of writing a full ProseMirror
 * node + paste rule, to keep the v1 surface manageable. The file
 * name (`milkdown-wikilink`) is retained for future migration when a
 * real ProseMirror plugin lands.
 */
export interface WikilinkLinkProps {
  slug: string
  pages: Record<string, WikiPageMeta>
  onResolve: (slug: string) => void
}

export function WikilinkLink({ slug, pages, onResolve }: WikilinkLinkProps) {
  const t = useT()
  const resolvable = Object.prototype.hasOwnProperty.call(pages, slug)
  if (resolvable) {
    return (
      <a
        href="#"
        data-testid={`wikilink-${slug}`}
        data-state="resolvable"
        className={cn(
          "text-accent hover:underline",
          "focus:outline-none focus:ring-2 focus:ring-accent-ring",
        )}
        onClick={(e) => {
          e.preventDefault()
          onResolve(slug)
        }}
      >
        [[{slug}]]
      </a>
    )
  }
  return (
    <span
      data-testid={`wikilink-${slug}`}
      data-state="unresolvable"
      title={t("workspace.wiki.pageNotFound")}
      className="cursor-not-allowed text-fg-tertiary opacity-50"
    >
      [[{slug}]]
    </span>
  )
}

/**
 * Pre-process a markdown body by replacing each `[[slug]]` with a
 * standard markdown anchor pointing at the synthetic
 * `codebus://wiki/<slug>` scheme. Milkdown's commonmark preset then
 * renders these as anchors; `WikiPreview` attaches a DOM-level
 * click handler that intercepts the synthetic scheme and routes
 * back into the wiki store.
 */
export function transformBodyWikilinks(body: string): {
  transformed: string
  slugs: string[]
} {
  const slugs: string[] = []
  const transformed = body.replace(
    /\[\[([^\]\n]+)\]\]/g,
    (_match, raw: string) => {
      const slug = raw.trim()
      slugs.push(slug)
      return `[${slug}](codebus://wiki/${encodeURIComponent(slug)})`
    },
  )
  return { transformed, slugs }
}
