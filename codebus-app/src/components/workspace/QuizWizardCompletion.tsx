import { CheckCircle2, XCircle } from "lucide-react"

import { Button } from "@/components/ui/button"
import { useT } from "@/i18n/useT"

export interface QuizCompletionResult {
  score: number
  total: number
  wrong: number[]
}

export interface QuizWizardCompletionProps {
  topic: string
  result: QuizCompletionResult
  passed: boolean
  threshold: number
  onRedo: () => void
  onViewWrong: () => void
  onViewProcess: () => void
}

/**
 * Step 4c of the Quiz wizard — completion summary.
 *
 * Spec: app-workspace § Quiz Tab Wizard Content Header And Layout
 * "Completion sub-state header is a back link plus result title". The
 * back-to-history affordance is supplied by the wizard TabContentHeader
 * (mock v1.1 §3.6), NOT this body.
 *
 * Pass branch surfaces "view process" (generation log); fail branch
 * surfaces "view wrong" (review the missed questions). The two are
 * intentionally asymmetric per mock §3.6.
 */
export function QuizWizardCompletion({
  topic,
  result,
  passed,
  threshold,
  onRedo,
  onViewWrong,
  onViewProcess,
}: QuizWizardCompletionProps) {
  const t = useT()
  const percent =
    result.total > 0 ? Math.round((result.score / result.total) * 100) : 0

  return (
    <div className="mx-auto flex w-full max-w-2xl flex-col items-center gap-4 px-6 py-12">
      <p className="text-meta text-fg-tertiary">{topic}</p>

      {passed ? (
        <div
          data-testid="quiz-wizard-completion-hero-pass"
          className="text-green-500"
        >
          <CheckCircle2 size={72} strokeWidth={1.5} />
        </div>
      ) : (
        <div
          data-testid="quiz-wizard-completion-hero-fail"
          className="text-red-500"
        >
          <XCircle size={72} strokeWidth={1.5} />
        </div>
      )}

      <h2 className="text-h-section font-semibold text-fg-primary">
        {passed
          ? t("workspace.quiz.wizard.step4.completionTitle.pass", { percent })
          : t("workspace.quiz.wizard.step4.completionTitle.fail", { percent })}
      </h2>
      <p className="text-meta text-fg-secondary">
        {result.score} / {result.total} · threshold {threshold}%
      </p>

      <div className="mt-6 flex items-center gap-2">
        <Button
          variant="primary"
          onClick={onRedo}
          data-testid="quiz-wizard-completion-redo"
        >
          {t("workspace.quiz.wizard.action.redo")}
        </Button>
        {passed ? (
          <Button
            variant="secondary"
            onClick={onViewProcess}
            data-testid="quiz-wizard-completion-view-process"
          >
            {t("workspace.quiz.wizard.action.viewProcess")}
          </Button>
        ) : (
          <Button
            variant="secondary"
            onClick={onViewWrong}
            data-testid="quiz-wizard-completion-view-wrong"
          >
            {t("workspace.quiz.wizard.action.viewWrong")}
          </Button>
        )}
      </div>

      {result.wrong.length > 0 && (
        <p
          data-testid="quiz-wizard-completion-wrong-list"
          className="mt-4 font-mono text-meta text-fg-tertiary"
        >
          wrong: {result.wrong.map((q) => `Q${q}`).join(" · ")}
        </p>
      )}
    </div>
  )
}
