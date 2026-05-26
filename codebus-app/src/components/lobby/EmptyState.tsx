import { useT } from "@/i18n/useT"
import type { Locale } from "@/hooks/useLocale"
import { Button } from "@/components/ui/button"

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
      <div className="text-[56px]" aria-hidden="true">
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
        <div className="text-fg-tertiary text-micro font-semibold uppercase tracking-[0.12em]">
          {t("lobby.empty.quickstartLabel")}
        </div>
        <ol className="mt-2 space-y-2 text-xs text-fg">
          {(
            [
              "lobby.empty.step1",
              "lobby.empty.step2",
              "lobby.empty.step3",
            ] as const
          ).map((key, i) => (
            <li key={key} className="flex gap-2">
              <span className="font-mono text-fg-tertiary">{i + 1}.</span>
              <span>{t(key)}</span>
            </li>
          ))}
        </ol>
      </div>
    </section>
  )
}
