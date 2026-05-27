import { useState } from "react"

import { Button } from "@/components/ui/button"
import { useT } from "@/i18n/useT"

export interface QuizWizardTopicProps {
  /** Up to ~5 wiki page titles surfaced as click-to-fill pills. */
  examplePills: string[]
  /** Submit a non-empty trimmed topic. Empty input is rejected inline. */
  onSubmit: (topic: string) => void
  /** Cancel control rendered alongside Next. */
  onCancel?: () => void
}

/**
 * Step 1 of the Quiz wizard — topic input.
 *
 * Spec: app-workspace § Quiz Tab Wizard Content Header And Layout
 * "Empty topic submission shows inline validation".
 */
export function QuizWizardTopic({
  examplePills,
  onSubmit,
  onCancel,
}: QuizWizardTopicProps) {
  const t = useT()
  const [value, setValue] = useState("")
  const [invalid, setInvalid] = useState(false)

  function attemptSubmit() {
    const trimmed = value.trim()
    if (trimmed.length === 0) {
      setInvalid(true)
      return
    }
    setInvalid(false)
    onSubmit(trimmed)
  }

  function clearInvalidOnChange(next: string) {
    setValue(next)
    if (invalid && next.trim().length > 0) {
      setInvalid(false)
    }
  }

  return (
    <div className="mx-auto flex w-full max-w-2xl flex-col gap-4 px-6 py-10">
      <h2 className="text-h-section font-semibold text-fg-primary">
        {t("workspace.quiz.wizard.step1.title")}
      </h2>
      <p className="text-meta text-fg-secondary">
        {t("workspace.quiz.wizard.step1.subtitle")}
      </p>

      <div className="relative">
        <input
          data-testid="quiz-wizard-topic-input"
          data-invalid={invalid ? "true" : "false"}
          type="text"
          value={value}
          placeholder={t("workspace.quiz.wizard.step1.placeholder")}
          aria-invalid={invalid}
          onChange={(e) => clearInvalidOnChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault()
              attemptSubmit()
            }
          }}
          className={
            "w-full rounded-md border bg-bg-raised px-3 py-2 text-body text-fg-primary placeholder:text-fg-tertiary focus:outline-none focus:ring-2 focus:ring-accent-ring " +
            (invalid ? "border-accent" : "border-border")
          }
        />
        {invalid && (
          <div
            data-testid="quiz-wizard-topic-empty-tooltip"
            role="tooltip"
            className="mt-1 text-meta text-accent"
          >
            {t("workspace.quiz.wizard.step1.examplePillHint")}
          </div>
        )}
      </div>

      {examplePills.length > 0 && (
        <div
          data-testid="quiz-wizard-topic-pills"
          className="flex flex-wrap gap-2"
        >
          {examplePills.map((pill, idx) => (
            <button
              key={pill}
              type="button"
              data-testid={`quiz-wizard-topic-pill-${idx}`}
              onClick={() => clearInvalidOnChange(pill)}
              className="rounded-full border border-border bg-bg-raised px-3 py-1 text-meta text-fg-secondary hover:border-accent hover:text-fg-primary"
            >
              {pill}
            </button>
          ))}
        </div>
      )}

      <p className="text-meta text-fg-tertiary">
        {t("workspace.quiz.wizard.step1.examplePillHint")}
      </p>

      <div className="mt-6 flex items-center justify-end gap-2">
        {onCancel && (
          <Button
            variant="secondary"
            onClick={onCancel}
            data-testid="quiz-wizard-topic-cancel"
          >
            {t("workspace.quiz.wizard.action.cancel")}
          </Button>
        )}
        <Button
          variant="primary"
          onClick={attemptSubmit}
          data-testid="quiz-wizard-topic-next"
        >
          {t("workspace.quiz.wizard.action.next")}
        </Button>
      </div>
    </div>
  )
}
