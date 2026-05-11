import type { TFunction } from "@/i18n/useT"

/**
 * Relative time formatter for vault `last_opened` labels.
 * - <1 minute: "just now"
 * - <1 hour:  "Nm ago"
 * - <1 day:   "Nh ago"
 * - <30 days: "Nd ago"
 * - >=30 days: absolute date "YYYY-MM-DD"
 *
 * Takes a `t` function so the labels honor the active locale.
 */
export function formatLastOpened(
  iso: string,
  t: TFunction,
  now: Date = new Date(),
): string {
  const then = new Date(iso)
  if (Number.isNaN(then.getTime())) return iso
  const deltaMs = now.getTime() - then.getTime()
  const deltaMin = Math.floor(deltaMs / 60000)
  if (deltaMin < 1) return t("common.justNow")
  if (deltaMin < 60) return t("common.minutesAgo", { n: deltaMin })
  const deltaHour = Math.floor(deltaMin / 60)
  if (deltaHour < 24) return t("common.hoursAgo", { n: deltaHour })
  const deltaDay = Math.floor(deltaHour / 24)
  if (deltaDay < 30) return t("common.daysAgo", { n: deltaDay })
  return then.toISOString().slice(0, 10)
}
