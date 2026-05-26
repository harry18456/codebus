import { useSettingsStore } from "@/store/settings"

export type Locale = "zh" | "en"

/**
 * Active UI locale. Precedence (top-down, evaluated on every render):
 *   1. `override` argument — tests inject a deterministic locale here
 *   2. Settings store `app.locale_override` (non-null) — user choice
 *   3. `navigator.language` — `zh-*` (case-insensitive) → "zh", else "en"
 *
 * Reactive: zustand selector subscribes to the override path, so changing
 * the Settings dropdown re-renders every consumer of this hook without any
 * remount or page reload. See spec *Settings Language Override*.
 */
export function useLocale(override?: Locale): Locale {
  const storeOverride = useSettingsStore(
    (s) =>
      (s.config as { app?: { locale_override?: Locale | null } } | undefined)
        ?.app?.locale_override ?? null,
  )
  if (override === "zh" || override === "en") return override
  if (storeOverride === "zh" || storeOverride === "en") return storeOverride
  if (typeof navigator === "undefined" || !navigator) return "en"
  const lang = (navigator.language || "en").toLowerCase()
  return lang.startsWith("zh") ? "zh" : "en"
}
