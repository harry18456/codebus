/**
 * Generic horizontal phase indicator (formerly QuizTab's local `StepDots`).
 * Renders `total` dots; dots before `current` are tinted as completed, the
 * dot at `current` is the active marker, and remaining dots are outlined
 * placeholders.
 *
 * Used by:
 *   - QuizTab wizard (4 dots, total=4, current=1..4)
 *   - LoadingOverlay vault init progress (6 dots, total=6, current=1..6)
 *
 * `state` controls the active dot's styling:
 *   - "running" (default): amber accent + ring
 *   - "done": treated like a completed dot (no ring), used when the whole
 *     sequence has finished
 *   - "error": amber-warm `--color-warn` fill (no ring) — same visual
 *     language as the 02c Interrupted banner; used by LoadingOverlay's
 *     failure mode per design "Failure mode 用 amber-warm 而非 hard-fail
 *     red".
 */
export interface PhaseDotsProps {
  total: number
  current: number
  state?: "running" | "done" | "error"
  testId?: string
  /**
   * Name of the data-attribute that holds `current` (without the
   * `data-` prefix). Defaults to `phase`. Quiz wizard passes
   * `current-step` to keep its existing `data-current-step` selector.
   */
  currentAttrName?: string
  className?: string
}

export function PhaseDots({
  total,
  current,
  state = "running",
  testId,
  currentAttrName = "phase",
  className,
}: PhaseDotsProps) {
  const containerClass = ["flex items-center gap-1.5", className]
    .filter(Boolean)
    .join(" ")

  // The data attribute name must be lowercase per HTML spec; the React
  // type for unknown data-* attributes is `Record<string, unknown>` via
  // spread.
  const dataAttrs: Record<string, unknown> = {
    [`data-${currentAttrName}`]: current,
  }
  if (testId) dataAttrs["data-testid"] = testId

  return (
    <span className={containerClass} {...dataAttrs}>
      {Array.from({ length: total }, (_, i) => i + 1).map((n) => (
        <span
          key={n}
          className={dotClass(n, current, state)}
          data-phase-index={n}
          data-phase-state={dotState(n, current, state)}
        />
      ))}
    </span>
  )
}

function dotState(
  n: number,
  current: number,
  state: "running" | "done" | "error",
): "done" | "current" | "pending" | "error" {
  if (n < current) return "done"
  if (n === current) {
    if (state === "error") return "error"
    if (state === "done") return "done"
    return "current"
  }
  return "pending"
}

function dotClass(
  n: number,
  current: number,
  state: "running" | "done" | "error",
): string {
  const base = "inline-block h-[7px] w-[7px] rounded-full"
  const s = dotState(n, current, state)
  switch (s) {
    case "done":
      return `${base} bg-fg-tertiary`
    case "current":
      return `${base} bg-accent ring-2 ring-accent-tint`
    case "error":
      return `${base} bg-warn`
    case "pending":
      return `${base} border border-border-strong`
  }
}
