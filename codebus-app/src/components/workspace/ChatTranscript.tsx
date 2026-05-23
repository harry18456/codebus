import { Fragment, useEffect, useMemo, useState } from "react"
import ReactMarkdown, { type Components } from "react-markdown"
import remarkGfm from "remark-gfm"

import { isAppError, type VerbEvent } from "@/lib/ipc"
import { transformBodyWikilinks } from "@/lib/milkdown-wikilink"
import {
  useChatStore,
  type ChatTurn,
  type ChatTurnLive,
} from "@/store/chat"
import { useWikiStore } from "@/store/wiki"

import {
  ActivityStreamItem,
  foldTimeline,
} from "./ActivityStreamItem"

/**
 * Spec: app-workspace § Chat Activity Stream Reuse with Per-Turn Grouping.
 *
 * Renders the chat transcript as a stack of vertical turn blocks. Each block
 * shows the user prompt at the top and the assistant activity stream below
 * (reusing `ActivityStreamItem` + `ThoughtItem` + `foldTimeline` so the
 * Run Detail tool one-liner / thought-fold rendering stays single-source).
 *
 * Between turns we render a horizontal divider so the visual cadence matches
 * the spec ("turn-N user → assistant → divider → turn-N+1 user → ..."). The
 * active (streaming) turn — if any — pins to the bottom and runs the same
 * activity-stream renderer over `activeTurn.events` so tool_use one-liners
 * appear inline as they arrive.
 *
 * Auto-scroll rule: only follow new events to the bottom when the viewport
 * is already within 50px of the bottom (the user has "released" the scroll).
 * If the user has scrolled up beyond that zone, leave `scrollTop` alone so
 * they can read older context without being yanked away — the next event
 * will auto-scroll again once they scroll back into the bottom zone.
 *
 * Spec: app-workspace § Chat Assistant Message Markdown Rendering and Wiki
 * Citation Links. Assistant text chunks (folded `StreamEvent::Thought` runs)
 * are concatenated per turn and rendered via `react-markdown` so wiki/external
 * markdown links can intercept clicks. Tool one-liners and banners still go
 * through `ActivityStreamItem`. Promote pill + onboarding hint stay out of
 * scope (other tasks own them).
 *
 * Link routing inside the assistant markdown block (per spec
 * `Chat Assistant Message Markdown Rendering and Wiki Citation Links`):
 * - `href` starts with `codebus://wiki/<encoded>` (produced from `[[slug]]`
 *   via `transformBodyWikilinks`) → decode slug → look up in
 *   `useWikiStore.pages` → resolvable: `<button>` with `meta.title ?? slug`
 *   label, click calls `onWikiLinkClick(slug)` AND collapses chat widget;
 *   unresolvable: dimmed `<span>` with `title="Page not found"`, inert.
 * - `href` matches `^wiki\/(.+)\.md$` → capture-group is the slug; same
 *   resolvable / unresolvable flow but the rendered label preserves the
 *   author-provided link text (the `[label](...)` label) rather than
 *   substituting the page title.
 * - `href` matches `^https?:` → `<a>` with preventDefault, calls Tauri
 *   opener plugin (`openUrl`).
 * - Anything else (e.g., `src/auth/jwt.rs`) → inert `<span>`, no click.
 *
 * Callback contract: `onWikiLinkClick` receives the **slug**, never a raw
 * href. Workspace's `onSelectPage(slug)` is the production consumer.
 *
 * Plain-text wiki paths NOT wrapped in markdown link syntax are deliberately
 * left as inert prose — react-markdown does not auto-link bare paths.
 */

// Legacy wiki-href shape with capture group for the slug between `wiki/` AND
// the trailing `.md`. `transformBodyWikilinks` produces the modern
// `codebus://wiki/<encoded>` form (handled separately below); this regex
// preserves recognition of the older `[label](wiki/<path>.md)` form some
// agent outputs still emit.
const WIKI_HREF_RE = /^wiki\/(.+)\.md$/
const EXTERNAL_HREF_RE = /^https?:/i
// Synthetic scheme prefix produced by `transformBodyWikilinks`. The
// renderer's `urlTransform` passes it through unchanged so this prefix
// reaches the custom `a` override below.
const CODEBUS_WIKI_SCHEME = "codebus://wiki/"

interface ChatTranscriptProps {
  /**
   * Vault path forwarded to `acceptPromoteSuggestion` when the user clicks
   * the inline `[Promote to goal: ...]` pill. Optional so isolated wiki/link
   * tests can render without wiring promote; promote-flow tests MUST pass it.
   */
  vaultPath?: string
  /**
   * Wired by `Workspace` to switch the active tab to `wiki` and invoke
   * `useWikiStore.loadPage(vaultPath, slug)`. Optional so isolated tests +
   * pre-`Workspace`-integration snapshots still render; when absent the
   * wiki link click still collapses the chat widget but performs no
   * tab/page navigation.
   */
  onWikiLinkClick?: (slug: string) => void
  /**
   * Wired by `Workspace` to switch to the Goals tab + select the freshly
   * spawned run id after a successful Promote click. Optional so tests that
   * exercise only the local UI transitions can render without the routing
   * side-effect.
   */
  onPromoteSuccess?: (runId: string) => void
}

export function ChatTranscript({
  vaultPath,
  onWikiLinkClick,
  onPromoteSuccess,
}: ChatTranscriptProps = {}) {
  const turns = useChatStore((s) => s.turns)
  const activeTurn = useChatStore((s) => s.activeTurn)
  const promoteSuggestion = useChatStore((s) => s.promoteSuggestion)
  const dismissPromoteSuggestion = useChatStore((s) => s.dismissPromoteSuggestion)
  const acceptPromoteSuggestion = useChatStore((s) => s.acceptPromoteSuggestion)
  const [promoteError, setPromoteError] = useState<string | null>(null)

  // Spec: app-workspace § Chat Onboarding Hint and Placeholder.
  //
  // Render the onboarding hint whenever the transcript is empty (no completed
  // turns AND no active streaming turn). Manual UX feedback ruled out the
  // earlier per-vault `localStorage` gate: the hint conveys promote-suggestion
  // mechanics that the user wants reaffirmed at the start of every fresh
  // conversation (after `+ New chat` or after vault re-open), not just the
  // very first time per vault.
  const isEmpty = turns.length === 0 && activeTurn === null
  const showOnboardingHint = isEmpty

  // Reset any stale inline error when the suggestion itself is cleared (e.g.,
  // the user dismissed the pill, or the store reset on session change). This
  // also keeps a previous-attempt error from re-appearing if a brand-new
  // suggestion lands on the same component instance.
  useEffect(() => {
    if (!promoteSuggestion) setPromoteError(null)
  }, [promoteSuggestion])

  async function handlePromote() {
    if (!promoteSuggestion) return
    setPromoteError(null)
    try {
      const runId = await acceptPromoteSuggestion(vaultPath ?? "")
      // Store flips `expanded` to false on success (collapse widget). Route
      // the caller to RunDetailRunning via the optional callback — no fallback
      // routing here because Workspace owns tab + selectedRunId state.
      onPromoteSuccess?.(runId)
    } catch (err) {
      // Backend rejects with `AppError::Invalid { field: "active_runs" }`
      // when a goal is already running; surface a stable user-facing line
      // and keep the pill in the DOM so the user can retry.
      const message =
        isAppError(err) && err.kind === "invalid" && err.field === "active_runs"
          ? "Another goal is running. Wait for it to finish."
          : "Promote failed. Try again."
      setPromoteError(message)
    }
  }

  return (
    <div
      data-testid="chat-transcript"
      className="flex flex-1 flex-col gap-2 overflow-auto p-3"
    >
      {showOnboardingHint && <ChatOnboardingHint />}
      {turns.map((turn, i) => {
        const showPill =
          promoteSuggestion !== null && promoteSuggestion.turnIndex === i
        return (
          <Fragment key={`turn-${i}`}>
            <TurnBlock
              turn={turn}
              onWikiLinkClick={onWikiLinkClick}
              promotePill={
                showPill ? (
                  <PromotePill
                    reason={promoteSuggestion.reason}
                    onPromote={handlePromote}
                    onDismiss={dismissPromoteSuggestion}
                    error={promoteError}
                  />
                ) : null
              }
            />
            {i < turns.length - 1 && (
              <div
                data-testid="chat-turn-divider"
                role="separator"
                aria-orientation="horizontal"
                className="my-1 border-t border-border"
              />
            )}
          </Fragment>
        )
      })}
      {activeTurn && (
        <>
          {turns.length > 0 && (
            <div
              data-testid="chat-turn-divider"
              role="separator"
              aria-orientation="horizontal"
              className="my-1 border-t border-border"
            />
          )}
          <ActiveTurnBlock
            turn={activeTurn}
            onWikiLinkClick={onWikiLinkClick}
            promotePill={
              promoteSuggestion !== null &&
              promoteSuggestion.turnIndex === turns.length ? (
                <PromotePill
                  reason={promoteSuggestion.reason}
                  onPromote={handlePromote}
                  onDismiss={dismissPromoteSuggestion}
                  error={promoteError}
                />
              ) : null
            }
          />
        </>
      )}
    </div>
  )
}

interface TurnBlockProps {
  turn: ChatTurn
  onWikiLinkClick?: (slug: string) => void
  promotePill?: React.ReactNode
}

function TurnBlock({ turn, onWikiLinkClick, promotePill }: TurnBlockProps) {
  return (
    <div
      data-testid="chat-turn"
      className="flex flex-col gap-1"
    >
      <UserPrompt text={turn.userText} />
      <AssistantTimeline
        events={turn.events}
        onWikiLinkClick={onWikiLinkClick}
      />
      {turn.error && (
        <div
          data-testid="chat-turn-error"
          role="alert"
          className="rounded border border-error/40 bg-error/10 px-2 py-1 text-[11px] text-error"
        >
          {turn.error}
        </div>
      )}
      {promotePill}
    </div>
  )
}

interface ActiveTurnBlockProps {
  turn: ChatTurnLive
  onWikiLinkClick?: (slug: string) => void
  promotePill?: React.ReactNode
}

function ActiveTurnBlock({
  turn,
  onWikiLinkClick,
  promotePill,
}: ActiveTurnBlockProps) {
  return (
    <div
      data-testid="chat-turn-active"
      className="flex flex-col gap-1"
    >
      <UserPrompt text={turn.userText} />
      <AssistantTimeline
        events={turn.events}
        onWikiLinkClick={onWikiLinkClick}
      />
      {promotePill}
    </div>
  )
}

/**
 * Empty-transcript hint shown whenever the transcript has no completed
 * turns AND no active streaming turn.
 *
 * Spec: app-workspace § Chat Onboarding Hint and Placeholder. The hint MUST
 * convey (1) that the user can ask anything about the vault AND (2) both
 * promote paths — AI-driven Promote suggestions AND explicit user-driven
 * "ask AI to promote" requests. The English copy MUST contain the substrings
 * `"AI will suggest"` and `"ask AI to promote"`; locale switching + the
 * Traditional Chinese copy (`"主動建議"` / `"主動跟 AI 講"`) are wired in
 * task 7.2 alongside the i18n keys `chat.onboarding.hintEn` /
 * `chat.onboarding.hintTw`.
 *
 * Manual UX feedback ruled out the earlier per-vault `localStorage` gate:
 * the hint conveys promote-suggestion mechanics that the user wants
 * reaffirmed at the start of every fresh conversation (after `+ New chat`,
 * after vault re-open, etc.), not just the very first time per vault.
 */
function ChatOnboardingHint() {
  return (
    <div
      data-testid="chat-onboarding-hint"
      className="rounded-md border border-border bg-bg-elevated p-3 text-[12px] text-muted-fg"
    >
      {/* TODO(task 7.2): replace with t("chat.onboarding.hint") once locale */}
      {/* messages land; the en/tw substrings are spec-mandated either way. */}
      Ask anything about this vault. AI will suggest{" "}
      <span className="font-mono">[Promote to goal]</span> when a discussion is
      worth documenting — or you can ask AI to promote it yourself in plain
      language.
    </div>
  )
}

interface PromotePillProps {
  reason: string
  onPromote: () => void
  onDismiss: () => void
  error: string | null
}

/**
 * Inline pill rendered at the tail of an assistant message when the verb
 * lifecycle emitted a `PromoteSuggestion`. Clicking `[Promote to goal: ...]`
 * spawns a `goal` run seeded with the transcript dump; clicking `[Dismiss]`
 * drops the suggestion. The inline error slot surfaces `AppError::Invalid
 * { field: "active_runs" }` rejections so the user knows to wait for the
 * currently running goal before retrying.
 */
function PromotePill({ reason, onPromote, onDismiss, error }: PromotePillProps) {
  return (
    <div
      data-testid="promote-pill"
      className="mt-1 flex flex-wrap items-center gap-2 text-[12px]"
    >
      <button
        type="button"
        onClick={onPromote}
        className="rounded-md border border-accent/40 bg-accent/5 px-2 py-0.5 text-accent hover:bg-accent/10 focus:outline-none focus:ring-2 focus:ring-accent-ring"
      >
        {`[Promote to goal: ${reason}]`}
      </button>
      <button
        type="button"
        onClick={onDismiss}
        className="rounded-md border border-border px-2 py-0.5 text-muted-fg hover:bg-bg-elevated focus:outline-none focus:ring-2 focus:ring-accent-ring"
      >
        [Dismiss]
      </button>
      {error && (
        <span data-testid="promote-error" className="text-danger">
          {error}
        </span>
      )}
    </div>
  )
}

function UserPrompt({ text }: { text: string }) {
  return (
    <div
      data-testid="chat-turn-user"
      className="self-end max-w-[85%] rounded-md bg-accent/10 px-3 py-1.5 text-[13px] text-fg whitespace-pre-wrap"
    >
      {text}
    </div>
  )
}

interface AssistantTimelineProps {
  events: readonly VerbEvent[]
  onWikiLinkClick?: (slug: string) => void
}

function AssistantTimeline({ events, onWikiLinkClick }: AssistantTimelineProps) {
  const timeline = useMemo(() => foldTimeline(events), [events])
  if (timeline.length === 0) return null
  return (
    <div
      data-testid="chat-turn-assistant"
      className="flex flex-col gap-0.5"
    >
      {timeline.map((item, i) =>
        item.kind === "thought_block" ? (
          <AssistantMarkdownBlock
            key={i}
            text={item.text}
            onWikiLinkClick={onWikiLinkClick}
          />
        ) : (
          <ActivityStreamItem key={i} event={item.event} />
        ),
      )}
    </div>
  )
}

/**
 * Render one assistant text block via react-markdown with a custom anchor
 * renderer that routes wiki / external / inert link patterns per spec.
 *
 * The Tauri opener plugin is imported dynamically so unit tests can mock the
 * module without pulling in the Tauri runtime at module-load time.
 */
async function openExternalUrl(url: string): Promise<void> {
  try {
    const { openUrl } = await import("@tauri-apps/plugin-opener")
    await openUrl(url)
  } catch (err) {
    // Failing to open the browser should not crash the chat panel; log and
    // swallow so the user can still keep chatting.
    console.error("Failed to open external URL", url, err)
  }
}

interface AssistantMarkdownBlockProps {
  text: string
  onWikiLinkClick?: (slug: string) => void
}

function AssistantMarkdownBlock({
  text,
  onWikiLinkClick,
}: AssistantMarkdownBlockProps) {
  // Subscribe to the page index so wikilink resolvable / unresolvable
  // classification is reactive — when a goal finishes and `useWikiStore`
  // re-fires `listPages`, formerly-unresolvable links auto-upgrade to
  // clickable without a chat re-mount.
  const pages = useWikiStore((s) => s.pages)

  // Pre-process `[[slug]]` into the synthetic `codebus://wiki/<encoded>`
  // markdown link form so react-markdown sees a normal anchor; the custom
  // `a` override below routes the synthetic scheme back to a wikilink
  // click. Uses the same helper WikiPreview uses for consistency.
  const transformed = useMemo(
    () => transformBodyWikilinks(text).transformed,
    [text],
  )

  const components: Components = useMemo(
    () => ({
      // Minimal table styling so GFM tables render with visible cell
      // boundaries instead of flowing as undifferentiated text.
      table: ({ children }) => (
        <table className="my-2 border-collapse border border-border text-[12px]">
          {children}
        </table>
      ),
      th: ({ children }) => (
        <th className="border border-border bg-bg-secondary px-2 py-1 text-left font-medium">
          {children}
        </th>
      ),
      td: ({ children }) => (
        <td className="border border-border px-2 py-1 align-top">{children}</td>
      ),
      a: ({ href, children }) => {
        if (typeof href !== "string") {
          return <span data-testid="chat-inert-link">{children}</span>
        }

        // Branch 1: codebus://wiki/<encoded-slug> — produced by
        // `transformBodyWikilinks` from `[[slug]]` syntax. Look up the
        // decoded slug in `pages` and render resolvable / unresolvable.
        if (href.startsWith(CODEBUS_WIKI_SCHEME)) {
          const encoded = href.slice(CODEBUS_WIKI_SCHEME.length)
          let slug: string
          try {
            slug = decodeURIComponent(encoded)
          } catch {
            // Malformed percent-encoding falls through to inert rendering.
            return <span data-testid="chat-inert-link">{children}</span>
          }
          // For `[[slug]]` syntax (the only source of codebus:// hrefs),
          // the markdown children is just the raw slug — prefer the
          // page's frontmatter title for natural prose ("Authentication"
          // beats "modules/auth"). Falls back to slug when title empty.
          return renderWikiSlug({
            slug,
            pages,
            children,
            preferTitle: true,
            onWikiLinkClick,
          })
        }

        // Branch 2: legacy `wiki/<path>.md` markdown link form. The capture
        // group is the slug to look up. Route through the same
        // resolvable / unresolvable flow as the codebus-scheme branch so
        // both forms behave identically (and the callback always receives
        // a slug, never a raw href — see spec Chat Assistant Message
        // Markdown Rendering and Wiki Citation Links). Unlike the
        // codebus:// branch, the legacy form's `children` is the
        // agent-authored link label which carries intent (`[auth.md](...)`)
        // — preserve it over the page title.
        const legacyMatch = href.match(WIKI_HREF_RE)
        if (legacyMatch) {
          const slug = legacyMatch[1] ?? ""
          return renderWikiSlug({
            slug,
            pages,
            children,
            preferTitle: false,
            onWikiLinkClick,
          })
        }

        // Branch 3: external http(s) link — Tauri opener.
        if (EXTERNAL_HREF_RE.test(href)) {
          return (
            <a
              href={href}
              data-testid="chat-external-link"
              className="text-accent underline hover:text-accent-hover"
              onClick={(e) => {
                e.preventDefault()
                void openExternalUrl(href)
              }}
            >
              {children}
            </a>
          )
        }

        // Branch 4: other path patterns (e.g., `src/auth/jwt.rs`): inert.
        return <span data-testid="chat-inert-link">{children}</span>
      },
    }),
    [onWikiLinkClick, pages],
  )

  return (
    <div
      data-testid="chat-assistant-markdown"
      className="text-[13px] text-fg whitespace-pre-wrap"
    >
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        // react-markdown's default urlTransform strips schemes outside
        // its safelist (http/https/mailto). The synthetic
        // `codebus://wiki/<slug>` produced by `transformBodyWikilinks`
        // would otherwise be flattened to an empty href; return URLs
        // unchanged so the custom `a` renderer can route them.
        urlTransform={(url) => url}
        components={components}
      >
        {transformed}
      </ReactMarkdown>
    </div>
  )
}

interface RenderWikiSlugArgs {
  slug: string
  pages: Record<string, { slug: string; path: string; title: string }>
  children: React.ReactNode
  /**
   * When true, prefer `pages[slug].title` over `children` as the visible
   * label (used for the `codebus://` branch where `children` is just the
   * raw slug from `transformBodyWikilinks`). When false, preserve
   * `children` as the visible label (used for the legacy
   * `[label](wiki/<path>.md)` branch where the label carries author
   * intent).
   */
  preferTitle: boolean
  onWikiLinkClick?: (slug: string) => void
}

/**
 * Shared resolvable / unresolvable wikilink renderer used by both the
 * `codebus://wiki/<encoded>` branch (modern `[[slug]]` syntax) AND the
 * legacy `wiki/<path>.md` branch. Centralising the rendering keeps both
 * input shapes behaving identically — the callback always receives a
 * slug (per spec Chat Assistant Message Markdown Rendering and Wiki
 * Citation Links), and unresolvable slugs render as inert dim spans
 * with a `Page not found` tooltip.
 */
function renderWikiSlug({
  slug,
  pages,
  children,
  preferTitle,
  onWikiLinkClick,
}: RenderWikiSlugArgs) {
  const meta = pages[slug]
  if (!meta) {
    return (
      <span
        data-testid="chat-wiki-link-unresolvable"
        title="Page not found"
        className="cursor-not-allowed text-fg-tertiary opacity-50"
      >
        {children}
      </span>
    )
  }
  // Label selection:
  //   - `preferTitle=true` (codebus:// branch / `[[slug]]` syntax):
  //     `meta.title` when non-empty, else slug. Chat prose reads
  //     naturally ("Authentication" beats "modules/auth").
  //   - `preferTitle=false` (legacy `[label](wiki/<path>.md)` branch):
  //     preserve `children` — the agent-authored link label is
  //     intentional and replacing it would erase author intent.
  const label = preferTitle
    ? meta.title.trim() !== ""
      ? meta.title
      : slug
    : children
  return (
    <button
      type="button"
      data-testid="chat-wiki-link"
      className="text-accent underline hover:text-accent-hover focus:outline-none focus:ring-2 focus:ring-accent-ring"
      onClick={(e) => {
        e.preventDefault()
        onWikiLinkClick?.(slug)
        // Collapse the chat widget only when currently expanded so
        // tests / callers can probe the state transition without a
        // surprise re-expand.
        if (useChatStore.getState().expanded) {
          useChatStore.getState().toggleExpanded()
        }
      }}
    >
      {label}
    </button>
  )
}
