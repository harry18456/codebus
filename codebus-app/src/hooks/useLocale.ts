import { useMemo } from "react"

export type Locale = "zh" | "en"

/**
 * System locale detection. `zh-*` languages map to Chinese; everything else
 * falls back to English. v1 has no language switcher — `useLocale` is the
 * single seam tests can mock via a wrapper.
 */
export function useLocale(override?: Locale): Locale {
  return useMemo(() => {
    if (override) return override
    if (typeof navigator === "undefined") return "en"
    const lang = (navigator.language || "en").toLowerCase()
    return lang.startsWith("zh") ? "zh" : "en"
  }, [override])
}
