/**
 * Renders a quiz `## Explanation` string with its `[[slug]]` citations
 * as navigable wiki links (quiz-attempt-progress design D6).
 *
 * The codebus-quiz SKILL requires every explanation to cite its source
 * via `[[slug]]`. This component splits the explanation on those tokens
 * and renders each the SAME way the app's primary wiki content renderer
 * (`WikiPreview`) does — the page's frontmatter title (falling back to
 * the bare slug only when the page is unknown), never the `[[ ]]`
 * bracketed form. Resolvable citations are clickable and call
 * `onOpenWikiPage`; unresolvable ones are dimmed plain text. Shared by
 * `QuizAnswering` (post-submit, both outcomes) and `QuizReview`.
 */
import { Fragment, type ReactNode } from "react"

import type { WikiPageMeta } from "@/lib/ipc"
import { tStatic } from "@/i18n/useT"

interface ExplanationTextProps {
  text: string
  /** Wiki page index (keyed by slug) for resolvability + title. */
  pages: Record<string, WikiPageMeta>
  /** Invoked when a resolvable citation is activated. */
  onOpenWikiPage?: (slug: string) => void
}

const WIKILINK = /\[\[([^\]]+)\]\]/g

export function ExplanationText({
  text,
  pages,
  onOpenWikiPage,
}: ExplanationTextProps) {
  const parts: ReactNode[] = []
  let last = 0
  let i = 0
  for (const m of text.matchAll(WIKILINK)) {
    const start = m.index ?? 0
    if (start > last) parts.push(text.slice(last, start))
    const slug = m[1].trim()
    const meta = pages[slug]
    // Mirror WikiPreview: show the page title when known, else the
    // bare slug — never the `[[slug]]` bracketed form.
    if (meta !== undefined) {
      parts.push(
        <a
          key={`wl-${i++}`}
          href="#"
          data-testid={`wikilink-${slug}`}
          data-wikilink={slug}
          data-state="resolvable"
          onClick={(e) => {
            e.preventDefault()
            onOpenWikiPage?.(slug)
          }}
          style={{ color: "#7c8cff" }}
          className="hover:underline"
        >
          {meta.title ?? slug}
        </a>,
      )
    } else {
      parts.push(
        <span
          key={`wl-${i++}`}
          data-testid={`wikilink-${slug}`}
          data-wikilink={slug}
          data-state="unresolvable"
          title={tStatic("workspace.wiki.pageNotFound")}
          className="cursor-not-allowed text-fg-tertiary opacity-50"
        >
          {slug}
        </span>,
      )
    }
    last = start + m[0].length
  }
  if (last < text.length) parts.push(text.slice(last))

  return (
    <>
      {parts.map((p, idx) => (
        <Fragment key={idx}>{p}</Fragment>
      ))}
    </>
  )
}
