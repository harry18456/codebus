import { describe, expect, it } from "vitest"

import { messages } from "./messages"

/**
 * i18n Bundle Coverage Policy (app-shell spec): all user-facing strings
 * in the Quiz tab views (QuizAnswering, QuizReview, QuizTab) SHALL be
 * defined as bundle keys with parity across `en` and `zh`. This file
 * targets Cat B sweep keys that the previous workspace bundle did not
 * cover.
 */
const QUIZ_KEYS = [
  // ---- QuizAnswering ----
  "workspace.quiz.answering.questionCounter",
  "workspace.quiz.answering.parseEmpty",
  "workspace.quiz.answering.summaryHeading",
  "workspace.quiz.answering.scoreLine",
  "workspace.quiz.answering.outcomePassed",
  "workspace.quiz.answering.outcomeFailed",
  "workspace.quiz.answering.verdictCorrect",
  "workspace.quiz.answering.verdictIncorrect",
  "workspace.quiz.answering.submitButton",
  "workspace.quiz.answering.nextButton",
  "workspace.quiz.answering.finishButton",

  // ---- QuizReview ----
  "workspace.quiz.review.backToHistory",
  "workspace.quiz.review.redoButton",
  "workspace.quiz.review.viewLogButton",
  "workspace.quiz.review.viewLogClose",
  "workspace.quiz.review.summaryLine",
  "workspace.quiz.review.yourAnswerLine",
  "workspace.quiz.review.generationLogTitle",

  // ---- QuizTab ----
  "workspace.quiz.tab.heading",
  "workspace.quiz.tab.newButton",
  "workspace.quiz.tab.emptyHint",
  "workspace.quiz.tab.startButton",
  "workspace.quiz.tab.topicPlaceholder",
  "workspace.quiz.tab.backToHistoryShort",
  "workspace.quiz.tab.backToHistoryFull",
  "workspace.quiz.tab.planningStatus",
  "workspace.quiz.tab.generatingStatus",
  "workspace.quiz.tab.noMatchPrefix",
  "workspace.quiz.tab.errorPrefix",
  "workspace.quiz.tab.backButton",

  // ---- Content header row (Phase 4C) ----
  "workspace.quiz.headerTitle",
  "workspace.quiz.headerSubtitle",

  // ---- Wizard (Phase 5.4 quiz-fullscreen-wizard-view) ----
  "workspace.quiz.wizard.step1.title",
  "workspace.quiz.wizard.step1.subtitle",
  "workspace.quiz.wizard.step1.placeholder",
  "workspace.quiz.wizard.step1.examplePillHint",
  "workspace.quiz.wizard.step2.title",
  "workspace.quiz.wizard.step2.bucketLabel.concepts",
  "workspace.quiz.wizard.step2.bucketLabel.entities",
  "workspace.quiz.wizard.step2.bucketLabel.modules",
  "workspace.quiz.wizard.step2.bucketLabel.processes",
  "workspace.quiz.wizard.step2.bucketLabel.synthesis",
  "workspace.quiz.wizard.step3.title",
  "workspace.quiz.wizard.step3.generatingHint",
  "workspace.quiz.wizard.step4.pendingTitle",
  "workspace.quiz.wizard.step4.reviewingTitle",
  "workspace.quiz.wizard.step4.completionTitle.pass",
  "workspace.quiz.wizard.step4.completionTitle.fail",
  "workspace.quiz.wizard.action.cancel",
  "workspace.quiz.wizard.action.back",
  "workspace.quiz.wizard.action.next",
  "workspace.quiz.wizard.action.start",
  "workspace.quiz.wizard.action.submit",
  "workspace.quiz.wizard.action.retry",
  "workspace.quiz.wizard.action.redo",
  "workspace.quiz.wizard.action.viewWrong",
  "workspace.quiz.wizard.action.viewProcess",
  "workspace.quiz.wizard.header.stepLabel",
] as const

/**
 * Per spec quiz § Quiz Scope Plan Bucket Taxonomy: the five bucket
 * identifiers are Cat D — they MUST remain literal English strings in
 * every locale, including any i18n value that IS the identifier itself.
 * The surrounding human-readable label text (rendered via
 * `bucketLabel.*` keys above) MAY be localized; the identifier-typed
 * value below SHALL stay English.
 */
const WIZARD_BUCKET_IDENTIFIER_KEYS = [
  "workspace.quiz.wizard.step2.bucketIdentifier.concepts",
  "workspace.quiz.wizard.step2.bucketIdentifier.entities",
  "workspace.quiz.wizard.step2.bucketIdentifier.modules",
  "workspace.quiz.wizard.step2.bucketIdentifier.processes",
  "workspace.quiz.wizard.step2.bucketIdentifier.synthesis",
] as const

const WIZARD_BUCKET_IDENTIFIER_EXPECTED: Record<string, string> = {
  "workspace.quiz.wizard.step2.bucketIdentifier.concepts": "concepts",
  "workspace.quiz.wizard.step2.bucketIdentifier.entities": "entities",
  "workspace.quiz.wizard.step2.bucketIdentifier.modules": "modules",
  "workspace.quiz.wizard.step2.bucketIdentifier.processes": "processes",
  "workspace.quiz.wizard.step2.bucketIdentifier.synthesis": "synthesis",
}

describe("i18n_wizard_bucket_identifiers_are_english_in_every_locale", () => {
  for (const key of WIZARD_BUCKET_IDENTIFIER_KEYS) {
    for (const locale of ["en", "zh"] as const) {
      it(`${locale}: ${key} stays literal English`, () => {
        const value = (messages[locale] as Record<string, string>)[key]
        expect(value).toBe(WIZARD_BUCKET_IDENTIFIER_EXPECTED[key])
      })
    }
  }
})

const QUIZ_BUCKET_IDENTIFIER_KEYS = WIZARD_BUCKET_IDENTIFIER_KEYS

describe("i18n_quiz_wizard_bucket_identifier_keys_in_both_locales", () => {
  for (const key of QUIZ_BUCKET_IDENTIFIER_KEYS) {
    it(`en: ${key}`, () => {
      expect(messages.en).toHaveProperty(key)
    })
    it(`zh: ${key}`, () => {
      expect(messages.zh).toHaveProperty(key)
    })
  }
})

describe("i18n_quiz_keys_in_both_locales", () => {
  for (const key of QUIZ_KEYS) {
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
 * Content header row (Phase 4C): new Quiz header keys MUST be real
 * translations — never the key literal — so useT never falls back to
 * showing the i18n identifier.
 */
const PHASE_4C_QUIZ_KEYS = [
  "workspace.quiz.headerTitle",
  "workspace.quiz.headerSubtitle",
] as const

describe("i18n_phase_4c_quiz_header_keys_are_translated_not_key_literal", () => {
  for (const key of PHASE_4C_QUIZ_KEYS) {
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

describe("i18n_quiz_outcome_threshold_placeholder", () => {
  it("answering.outcomePassed contains {n} placeholder for threshold percentage", () => {
    for (const locale of ["en", "zh"] as const) {
      const value = (messages[locale] as Record<string, string>)[
        "workspace.quiz.answering.outcomePassed"
      ]
      expect(value).toContain("{n}")
    }
  })
  it("answering.outcomeFailed contains {n} placeholder for threshold percentage", () => {
    for (const locale of ["en", "zh"] as const) {
      const value = (messages[locale] as Record<string, string>)[
        "workspace.quiz.answering.outcomeFailed"
      ]
      expect(value).toContain("{n}")
    }
  })
})
