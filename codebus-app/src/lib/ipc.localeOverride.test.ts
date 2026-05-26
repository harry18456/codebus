import { describe, expect, it } from "vitest"

import { parseLocaleOverride, type GlobalConfig } from "./ipc"

/**
 * Schema-level guard for `app.locale_override`. The Rust side serializes
 * `GlobalConfig` as `serde_yaml::Value`, so the frontend takes responsibility
 * for shape validation: only `"zh"` / `"en"` / `null` are valid; an absent
 * key or `undefined` resolves to "auto-detect" (returned as `null`).
 *
 * Backed by spec ADDED requirement *Settings Language Override* and the
 * "Legacy config without locale_override round-trips safely" scenario.
 */
describe("parseLocaleOverride", () => {
  it("returns null for an empty config (no app.locale_override)", () => {
    expect(() => parseLocaleOverride({} as GlobalConfig)).not.toThrow()
    expect(parseLocaleOverride({} as GlobalConfig)).toBeNull()
  })

  it("returns null when app exists but locale_override is absent (legacy round-trip)", () => {
    const cfg = { app: { quiz: { pass_threshold: 80 } } } as GlobalConfig
    expect(parseLocaleOverride(cfg)).toBeNull()
  })

  it("returns null for explicit null", () => {
    const cfg = { app: { locale_override: null } } as GlobalConfig
    expect(parseLocaleOverride(cfg)).toBeNull()
  })

  it("accepts \"en\"", () => {
    const cfg = { app: { locale_override: "en" } } as GlobalConfig
    expect(parseLocaleOverride(cfg)).toBe("en")
  })

  it("accepts \"zh\"", () => {
    const cfg = { app: { locale_override: "zh" } } as GlobalConfig
    expect(parseLocaleOverride(cfg)).toBe("zh")
  })

  it("throws on an invalid string value like \"fr\"", () => {
    const cfg = {
      app: { locale_override: "fr" },
    } as unknown as GlobalConfig
    expect(() => parseLocaleOverride(cfg)).toThrow(/locale_override/)
  })

  it("throws on a non-string non-null value", () => {
    const cfg = {
      app: { locale_override: 42 },
    } as unknown as GlobalConfig
    expect(() => parseLocaleOverride(cfg)).toThrow(/locale_override/)
  })
})
