import { describe, expect, it } from "vitest"

import { messages } from "./messages"

/**
 * i18n Bundle Coverage Policy (app-shell spec): aria-label / title attr
 * used as accessible name SHALL go through the bundle. Where the same
 * accessibility concept appears in more than one component, the bundle
 * SHALL expose ONE shared key that all sites consume.
 */

const A11Y_KEYS = [
  // Dialog primitive close button
  "a11y.dialogClose",

  // Chat widget controls (Cat C)
  "chat.widget.aria.openChat",
  "chat.widget.aria.resizeChat",
  "chat.widget.aria.minimizeChat",
  "chat.widget.title.dragToResize",

  // Wiki tree toggle
  "workspace.wiki.toggleTreeAria",

  // Shared key — surfaced by every "page not found" wikilink tooltip
  // across ChatTranscript / ExplanationText / WikiPreview / milkdown
  "workspace.wiki.pageNotFound",
] as const

describe("i18n_a11y_keys_in_both_locales", () => {
  for (const key of A11Y_KEYS) {
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

describe("i18n_a11y_page_not_found_is_single_shared_key", () => {
  it("only one bundle key holds the 'Page not found' tooltip text", () => {
    const enEntries = Object.entries(
      messages.en as Record<string, string>,
    ).filter(([, value]) => value === "Page not found")
    expect(enEntries).toHaveLength(1)
    expect(enEntries[0]?.[0]).toBe("workspace.wiki.pageNotFound")
  })
})
