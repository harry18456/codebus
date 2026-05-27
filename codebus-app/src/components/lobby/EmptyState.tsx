import { useT } from "@/i18n/useT"
import type { Locale } from "@/hooks/useLocale"
import { Button } from "@/components/ui/button"
import { SectionLabel } from "@/components/ui/SectionLabel"

interface EmptyStateProps {
  onBoard: () => void
  localeOverride?: Locale
}

export function EmptyState({ onBoard, localeOverride }: EmptyStateProps) {
  const t = useT(localeOverride)

  return (
    <section
      data-testid="lobby-empty"
      className="flex w-full max-w-[440px] flex-col items-center gap-5 px-6"
    >
      {/* large glyph, intentionally outside type scale */}
      <div
        className="codebus-bus-idle text-[56px]"
        aria-hidden="true"
      >
        🚌
      </div>
      <h1 className="text-h-empty font-semibold tracking-tight">
        {t("lobby.empty.title")}
      </h1>
      <p className="text-body text-fg-secondary text-center">
        {t("lobby.empty.subtitle")}
      </p>
      <Button
        variant="primary"
        size="lg"
        onClick={onBoard}
        data-testid="empty-board-cta"
      >
        {t("lobby.empty.cta")}
      </Button>
      <div className="mt-4 w-full rounded-lg border border-border bg-bg-raised p-[14px_18px]">
        <SectionLabel className="w-full">
          {t("lobby.empty.quickstartLabel")}
        </SectionLabel>
        <ol className="mt-2 grid grid-cols-[22px_1fr] gap-x-2.5 gap-y-2 text-xs text-fg">
          <li className="contents">
            <span className="font-mono text-fg-tertiary text-meta">1</span>
            <span>{t("lobby.empty.step1")}</span>
          </li>
          <li className="contents">
            <span className="font-mono text-fg-tertiary text-meta">2</span>
            <span>
              {t("lobby.empty.step2")}{" "}
              <span className="inline-block rounded-sm border border-accent/20 bg-accent-tint px-1.5 py-px font-mono text-meta text-accent">
                {t("lobby.empty.step2Example")}
              </span>
            </span>
          </li>
          <li className="contents">
            <span className="font-mono text-fg-tertiary text-meta">3</span>
            <span>{t("lobby.empty.step3")}</span>
          </li>
        </ol>
      </div>
    </section>
  )
}
