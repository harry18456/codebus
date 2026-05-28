import { useEffect, useMemo } from "react"
import type { ComponentPropsWithoutRef, ReactNode } from "react"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"

import { Button } from "@/components/ui/button"
import { useT } from "@/i18n/useT"
import { openWikiInObsidian } from "@/lib/ipc"
import { transformBodyWikilinks } from "@/lib/milkdown-wikilink"
import { useWikiStore } from "@/store/wiki"
import { useWatcherEvent } from "@/hooks/useWatcherEvent"

import { WikiPageMetadataBar } from "./WikiPageMetadataBar"

/**
 * Extract the wiki slug from an absolute path emitted by the Rust
 * watcher. The wiki store keys pages by `path.file_stem()` (see
 * `codebus-app/src-tauri/src/ipc/wiki.rs::list_wiki_pages_impl`), so a
 * page like `.codebus/wiki/concepts/project-purpose.md` has slug
 * `project-purpose`, NOT `concepts/project-purpose`. Returns `null`
 * when the path does not live under `<vault>/.codebus/wiki/` or does
 * not end in `.md`.
 */
function slugFromWatcherPath(
  payloadPath: string,
  vaultPath: string,
): string | null {
  const normPayload = payloadPath.replace(/\\/g, "/")
  const wikiPrefix = `${vaultPath.replace(/\\/g, "/")}/.codebus/wiki/`
  if (!normPayload.startsWith(wikiPrefix)) return null
  const rel = normPayload.slice(wikiPrefix.length)
  const lastSlash = rel.lastIndexOf("/")
  const leaf = lastSlash >= 0 ? rel.slice(lastSlash + 1) : rel
  if (!leaf.endsWith(".md")) return null
  return leaf.slice(0, -".md".length)
}

interface WikiPreviewProps {
  vaultPath: string
  body: string | null
  /**
   * v3-app-quiz task 5.3 — invoked when the user clicks
   * `[Quiz me on this]` on a wiki **content** page preview. Receives
   * the current page path. Nav pages (`index.md` / `log.md`) never
   * render the control, so the callback is only ever called with a
   * content page path.
   */
  onQuizMeOnThis?: (pagePath: string) => void
  /**
   * WP2 design v1.1: invoked when the user clicks the authoring-goal
   * token in the page metadata bar. Receives the goal id (frontmatter
   * `goals[last]`). Workspace forwards this into the Goal Detail view.
   */
  onGoalClick?: (goalId: string) => void
  /**
   * WP5 design v1.1: invoked when the user clicks the "Run a goal"
   * link in the edit hint footer. Receives the prefilled goal text
   * (already in the form `修改 wiki/<rel-path> — `). Workspace forwards
   * this into the Goals tab + opens the existing NewGoalModal pre-filled.
   */
  onRequestNewGoal?: (prefilledText: string) => void
}

/** Nav pages are metadata, not content to quiz on (spec §4.5). */
function isNavPage(path: string): boolean {
  return path.endsWith("index.md") || path.endsWith("log.md")
}

/**
 * Read-only markdown preview for wiki pages.
 *
 * Spec: app-workspace § Wiki Tab with Collapsible File Tree +
 * § Wikilink Resolution and Click Behavior.
 *
 * The historical Milkdown integration (kept as dependency for a
 * future ProseMirror migration) was swapped out for `react-markdown`
 * to give the v1 preview real visual hierarchy with negligible
 * engineering cost — design § Risks explicitly named
 * `react-markdown + regex renderer` as the documented fallback
 * path. The editor is read-only by construction (no input handlers
 * mounted) so the `editable: () => false` invariant is trivially
 * satisfied.
 *
 * Wikilinks are pre-transformed to `codebus://wiki/<slug>` anchors
 * and intercepted in the custom `a` component below; the click
 * routes through `useWikiStore.loadPage` rather than navigating
 * the browser.
 */
export function WikiPreview({
  vaultPath,
  body,
  onQuizMeOnThis,
  onGoalClick,
  onRequestNewGoal,
}: WikiPreviewProps) {
  const loadPage = useWikiStore((s) => s.loadPage)
  const pages = useWikiStore((s) => s.pages)
  const currentPath = useWikiStore((s) => s.currentPath)
  const obsidianVaultId = useWikiStore((s) => s.obsidianVaultId)
  const t = useT()

  // Re-fetch the currently displayed page when the watcher reports a
  // content change for its exact path. Edits to other `.md` files are
  // ignored so off-page work doesn't churn the preview. Windows file-
  // lock races are handled with a single 500 ms retry per design D4.
  useEffect(
    () =>
      useWatcherEvent("wiki-page-changed", (payload) => {
        const changedSlug = slugFromWatcherPath(payload.path, vaultPath)
        if (!changedSlug) return
        // Invalidate the cached body for the changed page regardless of
        // whether it is the currently displayed one. Without this the
        // store's loadPage cache-check would short-circuit and the user
        // would see stale content the next time the page is viewed.
        useWikiStore.setState((state) => {
          if (state._bodyCache[changedSlug] === undefined) return {} as never
          const next = { ...state._bodyCache }
          delete next[changedSlug]
          return { _bodyCache: next } as never
        })
        // If this IS the currently displayed page, immediately re-fetch
        // so the preview updates without a manual refresh. Windows
        // file-lock races are absorbed by a single 500 ms retry per
        // design D4.
        if (currentPath !== changedSlug) return
        void loadPage(vaultPath, changedSlug).catch(() => {
          setTimeout(() => {
            void loadPage(vaultPath, changedSlug)
          }, 500)
        })
      }),
    [loadPage, vaultPath, currentPath],
  )

  const { transformed, slugs } = useMemo(
    () => transformBodyWikilinks(body ?? ""),
    [body],
  )

  // WP2 metadata bar + WP5 edit hint footer rely on the active page's
  // frontmatter projection (`goals[]` + `updated`) and its absolute
  // disk path (used to derive a vault-relative path for the edit-hint
  // NewGoalModal prefill). Both default safely when the page index has
  // not loaded yet OR the currentPath does not match an entry.
  const currentPageMeta =
    currentPath !== null ? pages[currentPath] : undefined
  const goalLast =
    currentPageMeta && (currentPageMeta.goals ?? []).length > 0
      ? (currentPageMeta.goals as string[])[
          (currentPageMeta.goals as string[]).length - 1
        ]
      : null
  const updatedIso = currentPageMeta?.updated ?? ""
  const wikilinkCount = slugs.length

  function handleRequestNewGoal() {
    if (!currentPageMeta || !onRequestNewGoal) return
    const normalizedAbs = currentPageMeta.path.replace(/\\/g, "/")
    const normalizedVault = vaultPath.replace(/\\/g, "/")
    const wikiRoot = `${normalizedVault}/.codebus/wiki/`
    const relPath = normalizedAbs.startsWith(wikiRoot)
      ? normalizedAbs.slice(wikiRoot.length)
      : `${currentPageMeta.slug}.md`
    onRequestNewGoal(`修改 wiki/${relPath} — `)
  }

  if (body === null) {
    // WP-empty-page design v1.1: vault has pages but no page is currently
    // selected → render a low-density hint card (📂 emoji + two text lines)
    // instead of the bare-bones empty div the v1 preview used.
    return (
      <div
        data-testid="wiki-preview"
        className="flex h-full w-full items-center justify-center overflow-auto p-6"
      >
        <div
          data-testid="wiki-unselected-hint"
          className="flex flex-col items-center gap-2 text-center"
        >
          <span aria-hidden="true" className="text-[36px] leading-none text-fg-quaternary">
            📂
          </span>
          <p className="text-body text-fg-secondary">
            {t("workspace.wiki.unselectedHint.title")}
          </p>
          <p className="text-meta text-fg-tertiary">
            {t("workspace.wiki.unselectedHint.subtitle")}
          </p>
        </div>
      </div>
    )
  }

  return (
    <div
      data-testid="wiki-preview"
      className="h-full w-full overflow-auto"
    >
      <div
        className="mx-auto max-w-[720px] px-10 py-10 text-body-lg leading-[1.7] text-fg"
        style={{
          fontFamily:
            '-apple-system, BlinkMacSystemFont, "Segoe UI", "Inter", "Helvetica Neue", Arial, "Noto Sans TC", sans-serif',
        }}
      >
        {currentPageMeta && (
          <WikiPageMetadataBar
            goalLast={goalLast}
            updatedIso={updatedIso}
            wikilinkCount={wikilinkCount}
            onGoalClick={(g) => onGoalClick?.(g)}
          />
        )}
        <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        // react-markdown's default urlTransform strips custom URL
        // schemes (anything that isn't http/https/relative). The
        // synthetic `codebus://wiki/<slug>` scheme produced by
        // `transformBodyWikilinks` needs to survive so the custom
        // `a` renderer below can route the click into the wiki store.
        urlTransform={(url) => url}
        components={{
          h1: ({ children }) => (
            <h1 className="mb-4 mt-8 border-b border-border pb-2 text-[2em] font-bold leading-tight tracking-tight text-fg first:mt-0">
              {children}
            </h1>
          ),
          h2: ({ children }) => (
            <h2 className="mb-3 mt-7 border-b border-border/50 pb-1 text-[1.55em] font-bold leading-tight tracking-tight text-fg">
              {children}
            </h2>
          ),
          h3: ({ children }) => (
            <h3 className="mb-2 mt-5 text-[1.25em] font-semibold leading-snug text-fg">
              {children}
            </h3>
          ),
          h4: ({ children }) => (
            <h4 className="mb-1 mt-4 text-[1.05em] font-semibold text-fg">
              {children}
            </h4>
          ),
          p: ({ children }) => (
            <p className="my-3 leading-[1.7]">{children}</p>
          ),
          a: ({ href, children }) => {
            if (href && href.startsWith("codebus://wiki/")) {
              const slug = decodeURIComponent(
                href.slice("codebus://wiki/".length),
              )
              const meta = pages[slug]
              const resolvable = meta !== undefined
              // Display the page's frontmatter title when known so the
              // reader sees `Authentication Module` rather than the raw
              // `auth` slug. Falls back to the slug when the page
              // index has not loaded yet OR the link points at a page
              // that no longer exists.
              const displayText = meta?.title ?? slug
              if (resolvable) {
                return (
                  <a
                    href="#"
                    data-wikilink={slug}
                    data-state="resolvable"
                    onClick={(e) => {
                      e.preventDefault()
                      void loadPage(vaultPath, slug)
                    }}
                    className="plain-wikilink text-fg underline decoration-border-strong underline-offset-[3px] transition-colors hover:text-accent hover:decoration-accent motion-reduce:transition-none"
                  >
                    {displayText}
                  </a>
                )
              }
              // Unresolvable: dimmed span, hover tooltip, click is no-op.
              return (
                <span
                  data-wikilink={slug}
                  data-state="unresolvable"
                  title={t("workspace.wiki.pageNotFound")}
                  className="cursor-not-allowed text-fg-tertiary opacity-50"
                >
                  {slug}
                </span>
              )
            }
            return (
              <a
                href={href}
                target="_blank"
                rel="noopener noreferrer"
                className="plain-wikilink text-fg underline decoration-border-strong underline-offset-[3px] transition-colors hover:text-accent hover:decoration-accent motion-reduce:transition-none"
              >
                {children}
              </a>
            )
          },
          ul: ({ children }) => (
            <ul className="my-2 list-disc space-y-1 pl-6">{children}</ul>
          ),
          ol: ({ children }) => (
            <ol className="my-2 list-decimal space-y-1 pl-6">{children}</ol>
          ),
          li: ({ children }) => <li className="leading-relaxed">{children}</li>,
          code: (props: ComponentPropsWithoutRef<"code"> & { inline?: boolean }) => {
            const { inline, children, className } = props
            if (inline) {
              return (
                <code className="rounded border border-border bg-bg-sunken px-1 py-0.5 font-mono text-meta text-fg">
                  {children}
                </code>
              )
            }
            return (
              <code className={`font-mono text-meta ${className ?? ""}`}>
                {children}
              </code>
            )
          },
          pre: ({ children }: { children?: ReactNode }) => (
            <pre className="my-3 overflow-auto rounded-md border border-border bg-bg-sunken p-3 text-meta leading-relaxed">
              {children}
            </pre>
          ),
          blockquote: ({ children }) => (
            <blockquote className="my-3 border-l-4 border-border pl-4 italic text-fg-secondary">
              {children}
            </blockquote>
          ),
          hr: () => <hr className="my-6 border-border" />,
          table: ({ children }) => (
            <div className="my-3 overflow-auto">
              <table className="w-full border-collapse text-meta">
                {children}
              </table>
            </div>
          ),
          thead: ({ children }) => (
            <thead className="bg-bg-sunken">{children}</thead>
          ),
          th: ({ children }) => (
            <th className="border border-border px-2 py-1 text-left font-semibold">
              {children}
            </th>
          ),
          td: ({ children }) => (
            <td className="border border-border px-2 py-1 align-top">
              {children}
            </td>
          ),
          strong: ({ children }) => (
            <strong className="font-semibold text-fg">{children}</strong>
          ),
          em: ({ children }) => <em className="italic">{children}</em>,
          img: ({ src, alt }) => (
            <img
              src={src}
              alt={alt}
              className="my-3 max-w-full rounded border border-border"
            />
          ),
        }}
      >
        {transformed}
        </ReactMarkdown>
        {currentPath && (obsidianVaultId !== null || !isNavPage(currentPath)) && (
          <div className="mt-10 flex gap-3 border-t border-border pt-5">
            {!isNavPage(currentPath) && (
              <Button
                data-testid="quiz-me-on-this"
                variant="primary"
                onClick={() => onQuizMeOnThis?.(currentPath)}
              >
                {t("workspace.wiki.quizMeOnThis")}
              </Button>
            )}
            {obsidianVaultId !== null && (
              <Button
                data-testid="open-in-obsidian"
                variant="secondary"
                onClick={() => void openWikiInObsidian(vaultPath, currentPath)}
              >
                {t("workspace.wiki.openInObsidian")}
              </Button>
            )}
          </div>
        )}
        {currentPath && !isNavPage(currentPath) && (
          <WikiEditHintFooter
            disabled={!onRequestNewGoal}
            onRunGoal={handleRequestNewGoal}
          />
        )}
      </div>
    </div>
  )
}

/**
 * Spec: app-workspace § Wiki Page Edit Hint Footer (WP5 design v1.1).
 *
 * Reads the localized hint template (which contains a `{linkLabel}`
 * placeholder), splits on the placeholder, and renders the link label
 * as an inline button that triggers the parent's `onRunGoal` handler.
 * Splitting client-side is necessary because `useT`'s `{n}` interpolator
 * returns a flat string and we need a clickable React node inside the
 * sentence.
 */
function WikiEditHintFooter({
  disabled,
  onRunGoal,
}: {
  disabled: boolean
  onRunGoal: () => void
}) {
  const t = useT()
  const template = t("workspace.wiki.editHint.text")
  const linkLabel = t("workspace.wiki.editHint.linkLabel")
  const placeholder = "{linkLabel}"
  const idx = template.indexOf(placeholder)
  const before = idx >= 0 ? template.slice(0, idx) : template
  const after = idx >= 0 ? template.slice(idx + placeholder.length) : ""
  return (
    <p
      data-testid="wiki-edit-hint-footer"
      className="mt-6 text-meta text-fg-tertiary"
    >
      {before}
      <button
        type="button"
        data-testid="wiki-edit-hint-link"
        disabled={disabled}
        onClick={onRunGoal}
        className="text-accent underline decoration-dotted underline-offset-[3px] hover:text-accent-hover disabled:cursor-not-allowed disabled:opacity-50"
      >
        {linkLabel}
      </button>
      {after}
    </p>
  )
}
