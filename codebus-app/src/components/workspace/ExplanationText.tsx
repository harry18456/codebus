/**
 * Shared inline markdown renderer for quiz text fragments.
 *
 * Supported syntax is intentionally narrow: inline code, strong,
 * emphasis, and `[[slug]]` wiki citations. Block markdown stays literal
 * so quiz stems, choices, and explanations cannot turn into block
 * layouts.
 */
import { Fragment, type ReactNode } from "react"

import { tStatic } from "@/i18n/useT"
import type { WikiPageMeta } from "@/lib/ipc"

interface InlineMarkdownTextProps {
  text: string
  /** Wiki page index (keyed by slug) for resolvability + title. */
  pages: Record<string, WikiPageMeta>
  /** Invoked when a resolvable citation is activated. */
  onOpenWikiPage?: (slug: string) => void
}

interface RenderContext {
  pages: Record<string, WikiPageMeta>
  onOpenWikiPage?: (slug: string) => void
  nextKey: () => string
}

function renderWikiLink(slug: string, ctx: RenderContext): ReactNode {
  const meta = ctx.pages[slug]
  const key = ctx.nextKey()
  if (meta !== undefined) {
    return (
      <a
        key={key}
        href="#"
        data-testid={`wikilink-${slug}`}
        data-wikilink={slug}
        data-state="resolvable"
        onClick={(e) => {
          e.preventDefault()
          ctx.onOpenWikiPage?.(slug)
        }}
        className="cite-link font-mono text-accent underline decoration-dashed underline-offset-[3px] hover:text-accent-hover"
      >
        {meta.title ?? slug}
      </a>
    )
  }
  return (
    <span
      key={key}
      data-testid={`wikilink-${slug}`}
      data-wikilink={slug}
      data-state="unresolvable"
      title={tStatic("workspace.wiki.pageNotFound")}
      className="cursor-not-allowed text-fg-tertiary opacity-50"
    >
      {slug}
    </span>
  )
}

function renderInline(input: string, ctx: RenderContext): ReactNode[] {
  const nodes: ReactNode[] = []
  let text = ""
  let i = 0

  function flushText() {
    if (text.length === 0) return
    nodes.push(text)
    text = ""
  }

  while (i < input.length) {
    if (input.startsWith("[[", i)) {
      const end = input.indexOf("]]", i + 2)
      if (end >= 0) {
        flushText()
        const slug = input.slice(i + 2, end).trim()
        nodes.push(renderWikiLink(slug, ctx))
        i = end + 2
        continue
      }
    }

    if (input.startsWith("```", i)) {
      text += "```"
      i += 3
      continue
    }

    if (input[i] === "`") {
      const end = input.indexOf("`", i + 1)
      if (end > i + 1) {
        flushText()
        nodes.push(
          <code
            key={ctx.nextKey()}
            className="rounded border border-border bg-bg-sunken px-1 py-0.5 font-mono text-meta text-fg"
          >
            {input.slice(i + 1, end)}
          </code>,
        )
        i = end + 1
        continue
      }
    }

    if (input.startsWith("**", i)) {
      const end = input.indexOf("**", i + 2)
      if (end > i + 2) {
        flushText()
        nodes.push(
          <strong key={ctx.nextKey()} className="font-semibold text-fg">
            {renderInline(input.slice(i + 2, end), ctx)}
          </strong>,
        )
        i = end + 2
        continue
      }
    }

    if (input[i] === "*" && input[i + 1] !== "*") {
      const end = input.indexOf("*", i + 1)
      if (end > i + 1) {
        flushText()
        nodes.push(
          <em key={ctx.nextKey()} className="italic">
            {renderInline(input.slice(i + 1, end), ctx)}
          </em>,
        )
        i = end + 1
        continue
      }
    }

    text += input[i]
    i += 1
  }

  flushText()
  return nodes
}

export function InlineMarkdownText({
  text,
  pages,
  onOpenWikiPage,
}: InlineMarkdownTextProps) {
  let key = 0
  const ctx: RenderContext = {
    pages,
    onOpenWikiPage,
    nextKey: () => `inline-${key++}`,
  }
  const parts = renderInline(text, ctx)

  return (
    <>
      {parts.map((part, idx) => (
        <Fragment key={idx}>{part}</Fragment>
      ))}
    </>
  )
}

export function ExplanationText(props: InlineMarkdownTextProps) {
  return <InlineMarkdownText {...props} />
}
