import { describe, expect, it } from "vitest"

import { messages } from "./messages"

/**
 * Spec / task 10.1: every workspace-namespaced message key SHALL
 * appear in both `en` and `zh` bundles so `useT` never falls back to
 * the literal key string for either locale.
 */
const WORKSPACE_KEYS = [
  "workspace.backToLobby",
  "workspace.tab.goals",
  "workspace.tab.wiki",
  "workspace.tab.quiz",
  "workspace.goals.newGoalButton",
  "workspace.goals.emptyHint",
  "workspace.goals.examplePlaceholder1",
  "workspace.goals.examplePlaceholder2",
  "workspace.goals.examplePlaceholder3",
  "workspace.goals.headerTitle",
  "workspace.goals.headerSubtitle",
  "workspace.goals.emptyHeroTitle",
  "workspace.goals.emptyHeroSubtitle",
  "workspace.goals.runningTailPending",
  "workspace.newGoalModal.title",
  "workspace.newGoalModal.placeholder",
  "workspace.newGoalModal.cancel",
  "workspace.newGoalModal.run",
  "workspace.newGoalModal.blockedHint",
  "workspace.runDetail.backLink",
  "workspace.runDetail.runningBadge",
  "workspace.runDetail.cancelButton",
  "workspace.runDetail.cancellingButton",
  "workspace.runDetail.doneBadge",
  "workspace.runDetail.coveredPagesLabel",
  "workspace.runDetail.coveredPagesEmpty",
  "workspace.runDetail.lintLabel",
  "workspace.runDetail.cancelledBadge",
  "workspace.runDetail.cancelledWarning",
  "workspace.runDetail.interruptedBadge",
  "workspace.runDetail.interruptedWarning",
  "workspace.runDetail.partialTimelineLabel",
  "workspace.runDetail.retryButton",
  "workspace.wiki.empty",
  "workspace.wiki.toggleTreeAria",
  "workspace.wiki.pageNotFound",
  "workspace.quiz.placeholder",
] as const

describe("i18n_has_workspace_messages_in_both_locales", () => {
  for (const key of WORKSPACE_KEYS) {
    it(`en: ${key}`, () => {
      expect(messages.en).toHaveProperty(key)
      const value = (messages.en as Record<string, string>)[key]
      expect(typeof value).toBe("string")
      expect(value.length).toBeGreaterThan(0)
    })
    it(`zh: ${key}`, () => {
      expect(messages.zh).toHaveProperty(key)
      const value = (messages.zh as Record<string, string>)[key]
      expect(typeof value).toBe("string")
      expect(value.length).toBeGreaterThan(0)
    })
  }
})

/**
 * Content header row (Phase 4C): new header / hero keys MUST be real
 * translations — never the key literal — so the runtime never falls back
 * to showing the i18n identifier to the user.
 */
const PHASE_4C_GOALS_KEYS = [
  "workspace.goals.headerTitle",
  "workspace.goals.headerSubtitle",
  "workspace.goals.emptyHeroTitle",
  "workspace.goals.emptyHeroSubtitle",
] as const

describe("i18n_phase_4c_goals_header_keys_are_translated_not_key_literal", () => {
  for (const key of PHASE_4C_GOALS_KEYS) {
    it(`en value differs from key: ${key}`, () => {
      const value = (messages.en as Record<string, string>)[key]
      expect(value).not.toBe(key)
    })
    it(`zh value differs from key: ${key}`, () => {
      const value = (messages.zh as Record<string, string>)[key]
      expect(value).not.toBe(key)
    })
  }
})

/**
 * `workspace.goals.runningTailPending` is the placeholder shown in the
 * Goals-list running row tail before the first non-thought stream event
 * arrives. The placeholder is pure punctuation (`…`, U+2026) so it
 * SHALL be identical across locales — translators MUST NOT alter it.
 */
describe("i18n_workspace_goals_runningTailPending_is_ellipsis_in_both_bundles", () => {
  it("en bundle value is exactly '…' (U+2026)", () => {
    const value = (messages.en as Record<string, string>)[
      "workspace.goals.runningTailPending"
    ]
    expect(value).toBe("…")
  })
  it("zh bundle value is exactly '…' (U+2026)", () => {
    const value = (messages.zh as Record<string, string>)[
      "workspace.goals.runningTailPending"
    ]
    expect(value).toBe("…")
  })
})
