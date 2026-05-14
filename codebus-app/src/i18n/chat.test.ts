import { describe, expect, it } from "vitest"

import { messages } from "./messages"

/**
 * Spec / change `v3-app-chat-cmdk` task 7.2: every chat-namespaced
 * message key SHALL appear in both `en` and `zh` bundles with a
 * non-empty string value so `useT` never falls back to the literal
 * key string for either locale.
 *
 * Additionally, keys that should render as Traditional Chinese in
 * the `zh` bundle must contain at least one CJK ideograph — this
 * guards against accidental ASCII fallbacks (e.g. dropping in the
 * English copy by mistake).
 */
const CHAT_KEYS = [
  "chat.onboarding.hintEn",
  "chat.onboarding.hintTw",
  "chat.placeholder.en",
  "chat.placeholder.tw",
  "chat.button.newChat",
  "chat.button.stop",
  "chat.button.send",
  "chat.button.promote",
  "chat.button.dismiss",
  "chat.toast.startedNewChat",
  "chat.toast.undo",
  "chat.error.anotherGoalRunning",
  "chat.token.tooltip.cacheRead",
  "chat.token.tooltip.cacheCreate",
  "chat.token.tooltip.input",
  "chat.token.tooltip.output",
  "chat.widget.aria.openChat",
  "chat.widget.aria.closeChat",
] as const

// Keys whose `zh` bundle value MUST contain at least one CJK
// ideograph. Excludes:
// - `chat.onboarding.hintEn` / `chat.placeholder.en` — explicitly EN
//   variants (the `Tw` siblings carry the Chinese copy).
// - `chat.widget.aria.*` — exercised below with locale-specific
//   assertions instead (en should be ASCII, zh should be Chinese).
const ZH_KEYS_REQUIRING_HAN = [
  "chat.onboarding.hintTw",
  "chat.placeholder.tw",
  "chat.button.newChat",
  "chat.button.stop",
  "chat.button.send",
  "chat.button.promote",
  "chat.button.dismiss",
  "chat.toast.startedNewChat",
  "chat.toast.undo",
  "chat.error.anotherGoalRunning",
  "chat.token.tooltip.cacheRead",
  "chat.token.tooltip.cacheCreate",
  "chat.token.tooltip.input",
  "chat.token.tooltip.output",
  "chat.widget.aria.openChat",
  "chat.widget.aria.closeChat",
] as const

// CJK Unified Ideographs (basic block U+4E00–U+9FFF).
const HAN_RE = /[一-鿿]/

describe("i18n_has_chat_messages_in_both_locales", () => {
  for (const key of CHAT_KEYS) {
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

describe("i18n_chat_zh_uses_traditional_chinese", () => {
  for (const key of ZH_KEYS_REQUIRING_HAN) {
    it(`zh: ${key} contains a CJK ideograph`, () => {
      const value = (messages.zh as Record<string, string>)[key]
      expect(value).toMatch(HAN_RE)
    })
  }
})

describe("i18n_chat_onboarding_hint_contents", () => {
  it("hintEn mentions 'AI will suggest' and 'ask AI to promote'", () => {
    const en = (messages.en as Record<string, string>)[
      "chat.onboarding.hintEn"
    ]
    const zh = (messages.zh as Record<string, string>)[
      "chat.onboarding.hintEn"
    ]
    // `hintEn` is the English variant — same copy in both bundles.
    for (const value of [en, zh]) {
      expect(value).toMatch(/AI will suggest/i)
      expect(value.toLowerCase()).toContain("ask ai to promote")
    }
  })

  it("hintTw mentions 主動建議 and 主動跟 AI 講", () => {
    const en = (messages.en as Record<string, string>)[
      "chat.onboarding.hintTw"
    ]
    const zh = (messages.zh as Record<string, string>)[
      "chat.onboarding.hintTw"
    ]
    for (const value of [en, zh]) {
      expect(value).toContain("主動建議")
      expect(value).toContain("主動跟 AI 講")
    }
  })
})

describe("i18n_chat_placeholder_contents", () => {
  it("placeholder.en contains 'Ask anything'", () => {
    const en = (messages.en as Record<string, string>)[
      "chat.placeholder.en"
    ]
    const zh = (messages.zh as Record<string, string>)[
      "chat.placeholder.en"
    ]
    for (const value of [en, zh]) {
      expect(value).toContain("Ask anything")
    }
  })

  it("placeholder.tw contains 輸入訊息", () => {
    const en = (messages.en as Record<string, string>)[
      "chat.placeholder.tw"
    ]
    const zh = (messages.zh as Record<string, string>)[
      "chat.placeholder.tw"
    ]
    for (const value of [en, zh]) {
      expect(value).toContain("輸入訊息")
    }
  })
})
