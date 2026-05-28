import { useT } from "@/i18n/useT"
import { formatLastOpened } from "@/lib/time"

/**
 * Spec: app-workspace § Wiki Page Metadata Bar (WP2 design v1.1 spec lock).
 *
 * Single-line bar above the markdown body, composed of up to three
 * segments separated by middle-dot:
 *
 *   Last updated by <goal> · <time-ago> · <N> sources
 *
 * Suppression rules (per spec scenarios):
 * - `goalLast == null` → "Last updated by" segment is omitted entirely.
 * - `updatedIso` does not parse to a valid date → time-ago segment omitted.
 * - `wikilinkCount < 1` → sources segment omitted entirely.
 * - All three suppressed → component renders nothing (returns null).
 *
 * Forbidden additions (per spec): tags, word count, view count, authors.
 */
export interface WikiPageMetadataBarProps {
  /** Last element of frontmatter.goals[]; null when goals is empty. */
  goalLast: string | null
  /** Frontmatter `updated` ISO timestamp (may be malformed). */
  updatedIso: string
  /** Count of `[[wikilink]]` occurrences in the page body. */
  wikilinkCount: number
  /** Invoked when the user clicks the goal name token. */
  onGoalClick: (goalId: string) => void
  /** Injected clock for tests; defaults to `new Date()`. */
  now?: Date
}

export function WikiPageMetadataBar({
  goalLast,
  updatedIso,
  wikilinkCount,
  onGoalClick,
  now,
}: WikiPageMetadataBarProps) {
  const t = useT()
  const showGoal = goalLast !== null && goalLast.length > 0
  const updatedDate = new Date(updatedIso)
  const updatedValid = !Number.isNaN(updatedDate.getTime())
  const timeAgo = updatedValid
    ? formatLastOpened(updatedIso, t, now ?? new Date())
    : null
  const showSources = wikilinkCount > 0

  if (!showGoal && !timeAgo && !showSources) {
    return null
  }

  const segments: React.ReactNode[] = []
  if (showGoal) {
    segments.push(
      <span key="goal" className="inline-flex items-center gap-1">
        <span>{t("workspace.wiki.metadata.lastUpdatedBy")}</span>
        <button
          type="button"
          data-testid="wiki-page-metadata-goal"
          onClick={() => onGoalClick(goalLast)}
          className="text-fg underline decoration-dotted decoration-amber-500 underline-offset-[3px] hover:text-accent"
        >
          {goalLast}
        </button>
      </span>,
    )
  }
  if (timeAgo) {
    segments.push(<span key="time">{timeAgo}</span>)
  }
  if (showSources) {
    segments.push(
      <span key="sources">
        {wikilinkCount} {t("workspace.wiki.metadata.sourcesSuffix")}
      </span>,
    )
  }

  return (
    <div
      data-testid="wiki-page-metadata-bar"
      className="flex flex-wrap items-center gap-x-2 border-b border-border pb-3 font-mono text-meta text-fg-tertiary"
    >
      {segments.map((seg, i) => (
        <span key={i} className="inline-flex items-center gap-2">
          {i > 0 && <span aria-hidden="true">·</span>}
          {seg}
        </span>
      ))}
    </div>
  )
}
