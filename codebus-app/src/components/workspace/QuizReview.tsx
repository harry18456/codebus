/**
 * Read-only review of a completed quiz attempt (quiz-attempt-progress
 * design D4/D5).
 *
 * Spec: app-workspace § Quiz History List — a completed attempt opens
 * this Review (each question with the user's chosen answer, the correct
 * answer, and the explanation); it SHALL NOT render the attempt as raw
 * markdown. It carries `[重做此份]` (reset this attempt's sidecar and
 * re-enter answering at Q1 with the SAME questions — never a re-spawn,
 * distinct from `+ New quiz`) and, only when the attempt has a non-null
 * `events_log`, the existing centered-modal view-generation-log
 * affordance (reusing `QuizGenerationLog`).
 */
import { useMemo, useState } from "react"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { isPassing, parseQuiz, type ChoiceKey } from "@/lib/quiz-parse"
import type { QuizProgress, WikiPageMeta } from "@/lib/ipc"
import { useT } from "@/i18n/useT"
import { QuizGenerationLog } from "./QuizGenerationLog"
import { ExplanationText } from "./ExplanationText"

const CHOICE_KEYS: ChoiceKey[] = ["A", "B", "C", "D"]

interface QuizReviewProps {
  quizMd: string
  progress: QuizProgress
  passThreshold: number
  vaultPath: string
  /** Non-null enables the centered-modal view-generation-log affordance. */
  eventsLog: string | null
  /** Reset this attempt's sidecar and re-enter answering at Q1. */
  onRedo: () => void
  onBack: () => void
  /** Wiki page index + navigate handler for explanation citations (D6). */
  pages?: Record<string, WikiPageMeta>
  onOpenWikiPage?: (slug: string) => void
}

export function QuizReview({
  quizMd,
  progress,
  passThreshold,
  vaultPath,
  eventsLog,
  onRedo,
  onBack,
  pages,
  onOpenWikiPage,
}: QuizReviewProps) {
  const t = useT()
  const questions = useMemo(() => parseQuiz(quizMd), [quizMd])
  const [logOpen, setLogOpen] = useState(false)

  const total = questions.length
  const correctCount = progress.answers.filter((a) => a.correct).length
  const pass = isPassing(correctCount, total, passThreshold)

  return (
    <div
      data-testid="quiz-review"
      className="flex flex-1 flex-col gap-3 overflow-auto"
    >
      <div className="flex items-center gap-2">
        <Button data-testid="quiz-attempt-back" onClick={onBack}>
          {t("workspace.quiz.review.backToHistory")}
        </Button>
        <Button
          variant="primary"
          data-testid="quiz-redo-this"
          onClick={onRedo}
        >
          {t("workspace.quiz.review.redoButton")}
        </Button>
        {eventsLog && (
          <Button
            variant="secondary"
            data-testid="quiz-view-log"
            onClick={() => setLogOpen(true)}
          >
            {t("workspace.quiz.review.viewLogButton")}
          </Button>
        )}
      </div>

      {total > 0 && (
        <p
          data-testid="quiz-review-summary"
          className={pass ? "text-green-500" : "text-red-500"}
        >
          {t("workspace.quiz.review.summaryLine", {
            correct: correctCount,
            total,
            percent: Math.round((correctCount / total) * 100),
            outcome: pass
              ? t("workspace.quiz.answering.outcomePassed", { n: passThreshold })
              : t("workspace.quiz.answering.outcomeFailed", { n: passThreshold }),
          })}
        </p>
      )}

      <ol className="flex flex-col gap-4">
        {questions.map((q, i) => {
          const qNum = i + 1
          const answer = progress.answers.find((a) => a.q === qNum)
          const userChoice = answer?.selected ?? null
          const isCorrect = answer?.correct ?? false
          return (
            <li
              key={qNum}
              data-testid="quiz-review-question"
              className="flex flex-col gap-2 rounded border border-border p-3"
            >
              <p className="text-body text-fg-secondary">
                {t("workspace.quiz.answering.questionCounter", {
                  n: qNum,
                  total,
                })}
              </p>
              <h3 className="text-body-lg text-fg-primary">{q.stem}</h3>
              <ul className="flex flex-col gap-1">
                {CHOICE_KEYS.map((k) => {
                  const isAnswer = k === q.answer
                  const isPicked = k === userChoice
                  return (
                    <li
                      key={k}
                      className={[
                        "rounded px-2 py-1 text-body-lg",
                        isAnswer ? "bg-green-500/15" : "",
                        isPicked && !isAnswer ? "bg-red-500/15" : "",
                      ].join(" ")}
                    >
                      {k}) {q.choices[k]}
                    </li>
                  )
                })}
              </ul>
              <p
                className={
                  isCorrect ? "text-green-500 text-body" : "text-red-500 text-body"
                }
              >
                {t("workspace.quiz.review.yourAnswerLine", {
                  selected: userChoice ?? "—",
                  correct: q.answer,
                })}
              </p>
              <p className="text-body-lg text-fg-secondary">
                <ExplanationText
                  text={q.explanation}
                  pages={pages ?? {}}
                  onOpenWikiPage={onOpenWikiPage}
                />
              </p>
            </li>
          )
        })}
      </ol>

      {eventsLog && (
        <Dialog open={logOpen} onOpenChange={(o) => setLogOpen(o)}>
          <DialogContent data-testid="quiz-view-log-modal">
            <DialogHeader>
              <DialogTitle>
                {t("workspace.quiz.review.generationLogTitle")}
              </DialogTitle>
            </DialogHeader>
            <div className="max-h-[60vh] overflow-auto">
              <QuizGenerationLog vaultPath={vaultPath} eventsLog={eventsLog} />
            </div>
            <DialogClose asChild>
              <Button variant="secondary" data-testid="quiz-view-log-close">
                {t("workspace.quiz.review.viewLogClose")}
              </Button>
            </DialogClose>
          </DialogContent>
        </Dialog>
      )}
    </div>
  )
}
