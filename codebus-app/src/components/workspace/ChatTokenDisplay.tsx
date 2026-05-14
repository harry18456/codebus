import { useChatStore } from "@/store/chat"

/**
 * Chat-widget header token-usage indicator. Renders the session-cumulative
 * `input + output` token total in compact `<N>k ↑` form, and exposes a
 * four-way breakdown (input / output / cache read / cache create) via the
 * native `title` attribute so hover OR keyboard focus reveals the detail
 * without us hand-rolling a popover.
 *
 * The indicator is *always* visible — including the fresh-session zero state
 * (`0 ↑`) — so the widget header keeps a stable layout across the session
 * lifetime. Persistence semantics match the `Chat Token Usage Display` spec:
 *
 *   - `useChatStore.tokensTotal` is the source of truth; it is updated by the
 *     store reducer on every `stream-event { kind: "usage" }` arrival, and
 *     reset by `newSession()` / `resetForVault()`.
 *   - The store's TokenUsage shape uses `cache_read_tokens` and
 *     `cache_write_tokens`; the spec labels the latter as "cache create"
 *     (the user-facing wording matches the Anthropic dashboard terminology).
 *   - Strings are hard-coded English in this task; task 7.2 (i18n) replaces
 *     them with `chat.token.tooltip.*` keys.
 */
export function ChatTokenDisplay() {
  const tokens = useChatStore((s) => s.tokensTotal)
  const total = tokens.input_tokens + tokens.output_tokens
  const formatted = formatTokens(total)
  const cacheRead = tokens.cache_read_tokens ?? 0
  const cacheCreate = tokens.cache_write_tokens ?? 0
  // Native `title` is single-string; we join with newlines so the browser
  // tooltip renders one breakdown line at a time on hover/focus. Tests query
  // by substring so the exact separator is incidental.
  const title = [
    `input: ${tokens.input_tokens}`,
    `output: ${tokens.output_tokens}`,
    `cache read: ${cacheRead}`,
    `cache create: ${cacheCreate}`,
  ].join("\n")
  return (
    <span
      data-testid="chat-token-display"
      title={title}
      tabIndex={0}
      className="cursor-help select-none font-mono text-[11px] text-fg-tertiary hover:text-fg-secondary focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-ring"
    >
      {formatted}
    </span>
  )
}

/**
 * Format a raw token count for the header indicator. Rules from the
 * `Chat Token Usage Display` requirement:
 *
 *   - `< 1000` → raw integer with arrow (`250 ↑`). Spec only mandates the
 *     `<10k` / `≥10k` split but `0 ↑` is called out as the zero case, so
 *     anything below 1k is rendered as-is — a uniform fall-through avoids a
 *     surprise `0.2k` rendering for 200 tokens.
 *   - `< 10k` → one decimal place (`3.4k ↑`).
 *   - `≥ 10k` → rounded integer (`36k ↑`).
 */
function formatTokens(n: number): string {
  if (n < 1000) return `${n} ↑`
  const k = n / 1000
  if (k < 10) return `${k.toFixed(1)}k ↑`
  return `${Math.round(k)}k ↑`
}
