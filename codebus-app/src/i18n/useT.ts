import { useCallback } from "react"

import { useLocale, type Locale } from "@/hooks/useLocale"

import { type MessageKey, messages } from "./messages"

/**
 * Replace `{name}` placeholders in `template` with values from `vars`.
 * Unknown placeholders are left as-is (useful for debugging — a missing
 * variable surfaces as a literal `{name}` rather than empty).
 */
function interpolate(
  template: string,
  vars?: Record<string, string | number>,
): string {
  if (!vars) return template
  return template.replace(/\{(\w+)\}/g, (raw, key: string) =>
    Object.prototype.hasOwnProperty.call(vars, key) ? String(vars[key]) : raw,
  )
}

export type TFunction = (
  key: MessageKey,
  vars?: Record<string, string | number>,
) => string

/**
 * `t` returns the message string for the active locale. The locale is
 * detected from `useLocale` (system locale, with optional override).
 * Missing keys fall back to English; unknown keys (would be a TS error
 * upstream) return the key itself as a last-resort fallback.
 */
export function useT(overrideLocale?: Locale): TFunction {
  const locale = useLocale(overrideLocale)
  return useCallback(
    (key, vars) => {
      const bundle = messages[locale] ?? messages.en
      const template: string = bundle[key] ?? messages.en[key] ?? key
      return interpolate(template, vars)
    },
    [locale],
  )
}

/**
 * Module-level `t` for code outside React (stores, helpers). Reads
 * `navigator.language` directly each call. Components should use `useT`
 * instead so the locale honors `localeOverride`.
 */
export function tStatic(
  key: MessageKey,
  vars?: Record<string, string | number>,
): string {
  const lang =
    typeof navigator !== "undefined" && navigator.language
      ? navigator.language.toLowerCase()
      : "en"
  const locale: Locale = lang.startsWith("zh") ? "zh" : "en"
  const bundle = messages[locale] ?? messages.en
  const template: string = bundle[key] ?? messages.en[key] ?? key
  return interpolate(template, vars)
}
