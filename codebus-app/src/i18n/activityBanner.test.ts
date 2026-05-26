import { describe, expect, it } from "vitest"

import { messages } from "./messages"

/**
 * i18n Bundle Coverage Policy · emoji-prefixed label scenario.
 * Asserts the 10 bannerLabel keys + 4 unit/verdict keys added in change
 * i18n-sweep-phase-3a-followup exist in both locales and that each
 * banner value embeds its emoji as one unit (not split into two keys).
 */
const BANNER_KEYS_WITH_EMOJI: Array<{ key: string; emoji: string }> = [
  { key: "workspace.activity.banner.start", emoji: "🚌" },
  { key: "workspace.activity.banner.goal", emoji: "🎯" },
  { key: "workspace.activity.banner.syncStart", emoji: "🔄" },
  { key: "workspace.activity.banner.syncDone", emoji: "✓" },
  { key: "workspace.activity.banner.piiSummary", emoji: "🛡" },
  { key: "workspace.activity.banner.lintStart", emoji: "🔍" },
  { key: "workspace.activity.banner.lintDone", emoji: "✓" },
  { key: "workspace.activity.banner.commitDone", emoji: "🚏" },
  { key: "workspace.activity.banner.done", emoji: "🎉" },
  { key: "workspace.activity.banner.hint", emoji: "💡" },
]

const UNIT_VERDICT_KEYS = [
  "quiz.badge.pass",
  "quiz.badge.fail",
  "workspace.run.lintSummary",
  "workspace.run.headerSummary",
] as const

describe("i18n_activity_banner_keys_present_in_both_locales", () => {
  for (const { key, emoji } of BANNER_KEYS_WITH_EMOJI) {
    it(`en: ${key} starts with ${emoji}`, () => {
      const value = (messages.en as Record<string, string>)[key]
      expect(value, `missing en value for ${key}`).toBeTruthy()
      expect(value.startsWith(emoji)).toBe(true)
    })
    it(`zh: ${key} starts with ${emoji}`, () => {
      const value = (messages.zh as Record<string, string>)[key]
      expect(value, `missing zh value for ${key}`).toBeTruthy()
      expect(value.startsWith(emoji)).toBe(true)
    })
  }
})

describe("i18n_unit_and_verdict_keys_present_in_both_locales", () => {
  for (const key of UNIT_VERDICT_KEYS) {
    it(`en: ${key}`, () => {
      const value = (messages.en as Record<string, string>)[key]
      expect(typeof value).toBe("string")
      expect(value.length).toBeGreaterThan(0)
    })
    it(`zh: ${key}`, () => {
      const value = (messages.zh as Record<string, string>)[key]
      expect(typeof value).toBe("string")
      expect(value.length).toBeGreaterThan(0)
    })
  }
})
