import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { useT } from "@/i18n/useT"

import type { Locale } from "@/hooks/useLocale"

/**
 * Settings modal language override dropdown.
 *
 * Backs spec MODIFIED requirement *Global Settings Modal Field Set* item 12
 * and ADDED requirement *Settings Language Override*. Three fixed options
 * map to store values:
 *   - "auto" → `null`  (auto-detect from navigator.language)
 *   - "zh"   → `"zh"`
 *   - "en"   → `"en"`
 *
 * The select trigger uses the same Radix `Select` primitive as the PII
 * scanner row directly above so the dropdown styling matches without any
 * one-off CSS. Identifier labels "中文" / "English" are sourced from the
 * i18n bundle but carry identical literal strings in both locales (Cat D
 * jargon policy).
 */
export interface LanguageSectionProps {
  /** Current store value: `"zh"` / `"en"` / `null` (auto). */
  value: Locale | null
  /** Called with the new store value when the user picks an option. */
  onChange: (next: Locale | null) => void
}

type DropdownValue = "auto" | "zh" | "en"

function toDropdown(value: Locale | null): DropdownValue {
  return value === "zh" || value === "en" ? value : "auto"
}

function fromDropdown(value: DropdownValue): Locale | null {
  return value === "auto" ? null : value
}

export function LanguageSection({ value, onChange }: LanguageSectionProps) {
  const t = useT()
  return (
    <Select
      value={toDropdown(value)}
      onValueChange={(v) => onChange(fromDropdown(v as DropdownValue))}
    >
      <SelectTrigger
        className="w-[200px]"
        data-testid="language-select-trigger"
      >
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="auto" data-testid="language-option-auto">
          {t("settings.language.auto")}
        </SelectItem>
        <SelectItem value="zh" data-testid="language-option-zh">
          {t("settings.language.zh")}
        </SelectItem>
        <SelectItem value="en" data-testid="language-option-en">
          {t("settings.language.en")}
        </SelectItem>
      </SelectContent>
    </Select>
  )
}
