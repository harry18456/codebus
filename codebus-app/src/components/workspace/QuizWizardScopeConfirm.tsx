import { useMemo, useState } from "react"

import { Button } from "@/components/ui/button"
import { BUCKET_IDS, type BucketId, type ScopeBuckets } from "@/store/quiz-wizard"
import { useT } from "@/i18n/useT"

export interface QuizWizardScopeConfirmProps {
  /** LLM-planned page list grouped by the Karpathy 5 buckets. */
  buckets: ScopeBuckets
  /**
   * Fires when the user activates the confirm control. Receives the
   * English bucket identifiers the user kept selected, in
   * `BUCKET_IDS` display order. Identifiers are Cat D — never
   * translated, see quiz § Quiz Scope Plan Bucket Taxonomy.
   */
  onConfirm: (selectedIds: BucketId[]) => void
  /** Back to topic step — discards the planned buckets. */
  onBack: () => void
}

/**
 * Step 2 of the Quiz wizard — scope confirm. Renders the five buckets in
 * the fixed display order required by spec; user MAY deselect any bucket
 * to exclude it from the generate payload.
 */
export function QuizWizardScopeConfirm({
  buckets,
  onConfirm,
  onBack,
}: QuizWizardScopeConfirmProps) {
  const t = useT()
  const [deselected, setDeselected] = useState<Set<BucketId>>(new Set())

  function toggleBucket(id: BucketId) {
    setDeselected((prev) => {
      const next = new Set(prev)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
      }
      return next
    })
  }

  const selectedIds = useMemo<BucketId[]>(
    () => BUCKET_IDS.filter((id) => !deselected.has(id)),
    [deselected],
  )

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-4 px-6 py-8">
      <h2 className="text-h-section font-semibold text-fg-primary">
        {t("workspace.quiz.wizard.step2.title")}
      </h2>

      <div className="flex flex-col gap-3">
        {BUCKET_IDS.map((id) => {
          const pages = buckets[id]
          const isSelected = !deselected.has(id)
          const labelKey = `workspace.quiz.wizard.step2.bucketLabel.${id}` as const
          return (
            <section
              key={id}
              data-testid={`quiz-wizard-bucket-${id}`}
              data-bucket-id={id}
              className={
                "rounded-md border p-3 " +
                (isSelected
                  ? "border-border bg-bg-raised"
                  : "border-border opacity-50")
              }
            >
              <div className="flex items-center justify-between">
                <h3 className="text-body font-medium text-fg-primary">
                  {t(labelKey)}
                </h3>
                <button
                  type="button"
                  data-testid={`quiz-wizard-bucket-toggle-${id}`}
                  onClick={() => toggleBucket(id)}
                  className="text-meta text-fg-tertiary hover:text-fg-secondary"
                >
                  {isSelected
                    ? t("workspace.quiz.wizard.action.cancel")
                    : t("workspace.quiz.wizard.action.next")}
                </button>
              </div>
              {pages.length === 0 ? (
                <p className="mt-2 text-meta text-fg-tertiary italic">—</p>
              ) : (
                <ul className="mt-2 flex flex-col gap-1">
                  {pages.map((page) => (
                    <li
                      key={page}
                      className="font-mono text-meta text-fg-secondary"
                    >
                      {page}
                    </li>
                  ))}
                </ul>
              )}
            </section>
          )
        })}
      </div>

      <div className="mt-4 flex items-center justify-end gap-2">
        <Button
          variant="secondary"
          onClick={onBack}
          data-testid="quiz-wizard-scope-back"
        >
          {t("workspace.quiz.wizard.action.back")}
        </Button>
        <Button
          variant="primary"
          onClick={() => onConfirm(selectedIds)}
          data-testid="quiz-wizard-scope-confirm"
        >
          {t("workspace.quiz.wizard.action.start")}
        </Button>
      </div>
    </div>
  )
}
