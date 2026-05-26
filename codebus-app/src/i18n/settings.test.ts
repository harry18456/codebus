import { describe, expect, it } from "vitest"

import { messages } from "./messages"

/**
 * i18n Bundle Coverage Policy (app-shell spec): all user-facing strings
 * in the Settings panel (EndpointSection, CodexEndpointSection,
 * SetKeyDialog) SHALL be defined as keys in `messages.ts` with parity
 * across `en` and `zh`. Jargon allow-list keys (base_url / api_version /
 * keyring_service / verb names / codex effort) SHALL have IDENTICAL en
 * and zh values.
 */

const TRANSLATABLE_SETTINGS_KEYS = [
  // ---- Section headings ----
  "settings.endpoint.claude.heading",
  "settings.endpoint.codex.heading",

  // ---- Active-profile radio ----
  "settings.endpoint.activeProfileAria",
  "settings.endpoint.activeProfileAriaCodex",
  "settings.endpoint.profile.system",
  "settings.endpoint.profile.azure",
  "settings.endpoint.profile.systemTitle",
  "settings.endpoint.profile.azureTitle",
  "settings.endpoint.profile.inactiveLabel",

  // ---- Field labels (non-jargon) ----
  "settings.endpoint.field.apiKey",
  "settings.endpoint.field.effort",

  // ---- Placeholders ----
  "settings.endpoint.placeholder.claudeModel",
  "settings.endpoint.placeholder.codexModel",
  "settings.endpoint.placeholder.deploymentName",
  "settings.endpoint.placeholder.azureBaseUrlClaude",
  "settings.endpoint.placeholder.azureBaseUrlCodex",
  "settings.endpoint.placeholder.apiVersion",
  "settings.endpoint.placeholder.codexEffort",

  // ---- Key status / actions ----
  "settings.endpoint.keyStatus.set",
  "settings.endpoint.keyStatus.unset",
  "settings.endpoint.keyStatus.unknown",
  "settings.endpoint.keySetNew",
  "settings.endpoint.keyDelete",

  // ---- Validation summary ----
  "settings.endpoint.validationSummaryHeading",

  // ---- SetKeyDialog ----
  "settings.setKeyDialog.title",
  "settings.setKeyDialog.inputLabel",
  "settings.setKeyDialog.errorEmpty",
] as const

/**
 * Jargon allow-list keys — both locales MUST share the literal English
 * jargon string (e.g. `base_url`, `goal`, `low`). The Cat D policy
 * requires the bundle to hold these for centralization even though no
 * translation occurs.
 */
const JARGON_KEYS_EXPECTED_VALUE = {
  // Config YAML key names
  "settings.endpoint.field.baseUrl": "base_url",
  "settings.endpoint.field.apiVersion": "api_version",
  "settings.endpoint.field.keyringService": "keyring_service",

  // Verb names (CLI verb identifiers)
  "settings.endpoint.verb.goal": "goal",
  "settings.endpoint.verb.query": "query",
  "settings.endpoint.verb.fix": "fix",
  "settings.endpoint.verb.verify": "verify",

  // Codex effort enum
  "settings.endpoint.codex.effort.low": "low",
  "settings.endpoint.codex.effort.medium": "medium",
  "settings.endpoint.codex.effort.high": "high",
  "settings.endpoint.codex.effort.xhigh": "xhigh",
} as const

describe("i18n_settings_endpoint_keys_in_both_locales", () => {
  for (const key of TRANSLATABLE_SETTINGS_KEYS) {
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

describe("i18n_settings_jargon_keys_identical_en_and_zh", () => {
  for (const [key, expectedValue] of Object.entries(
    JARGON_KEYS_EXPECTED_VALUE,
  )) {
    it(`en value === jargon literal: ${key}`, () => {
      expect((messages.en as Record<string, string>)[key]).toBe(expectedValue)
    })
    it(`zh value === en value (jargon held identical): ${key}`, () => {
      expect((messages.zh as Record<string, string>)[key]).toBe(expectedValue)
    })
  }
})

describe("i18n_settings_endpoint_incompleteness_heading_localized", () => {
  it("en heading retains English text", () => {
    expect(
      (messages.en as Record<string, string>)[
        "settings.endpoint.validationSummaryHeading"
      ],
    ).toBe("Endpoint configuration is incomplete:")
  })
  it("zh heading translates the sentence (jargon limited to yaml key names within messages)", () => {
    expect(
      (messages.zh as Record<string, string>)[
        "settings.endpoint.validationSummaryHeading"
      ],
    ).toBe("端點設定不完整：")
  })
})

describe("i18n_settings_language_keys_present_in_both_locales", () => {
  const KEYS = [
    "settings.language.label",
    "settings.language.auto",
    "settings.language.zh",
    "settings.language.en",
  ] as const
  for (const key of KEYS) {
    it(`en: ${key}`, () => {
      expect(messages.en).toHaveProperty(key)
      expect(typeof (messages.en as Record<string, string>)[key]).toBe("string")
      expect((messages.en as Record<string, string>)[key].length).toBeGreaterThan(0)
    })
    it(`zh: ${key}`, () => {
      expect(messages.zh).toHaveProperty(key)
      expect(typeof (messages.zh as Record<string, string>)[key]).toBe("string")
      expect((messages.zh as Record<string, string>)[key].length).toBeGreaterThan(0)
    })
  }
})

describe("i18n_settings_language_identifier_labels_not_translated", () => {
  it("settings.language.zh is identical '中文' in both locales", () => {
    expect((messages.en as Record<string, string>)["settings.language.zh"]).toBe(
      "中文",
    )
    expect((messages.zh as Record<string, string>)["settings.language.zh"]).toBe(
      "中文",
    )
  })
  it("settings.language.en is identical 'English' in both locales", () => {
    expect((messages.en as Record<string, string>)["settings.language.en"]).toBe(
      "English",
    )
    expect((messages.zh as Record<string, string>)["settings.language.en"]).toBe(
      "English",
    )
  })
})

describe("i18n_settings_provider_cli_install_hint_localized_via_useT_template", () => {
  it("en template substitutes {name}", () => {
    const tpl = (messages.en as Record<string, string>)[
      "settings.providerCli.installHint"
    ]
    expect(tpl).toBeDefined()
    expect(tpl.replace(/\{name\}/g, "Claude Code")).toBe(
      "Install Claude Code first; then reopen Settings.",
    )
  })
  it("zh template substitutes {name}", () => {
    const tpl = (messages.zh as Record<string, string>)[
      "settings.providerCli.installHint"
    ]
    expect(tpl).toBeDefined()
    expect(tpl).toContain("{name}")
    const filled = tpl.replace(/\{name\}/g, "Claude Code")
    expect(filled).not.toContain("{name}")
    expect(filled.length).toBeGreaterThan(0)
  })
})
