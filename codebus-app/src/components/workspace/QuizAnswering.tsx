/**
 * Quiz answering view (v3-app-quiz task 5.4).
 *
 * Spec: app-workspace § Quiz Answering and Summary.
 *
 * One question per screen, four choices. On submit the answer is graded
 * **client-side** by comparing the selection to the parsed `Answer`
 * field — there is NO agent spawn. An incorrect answer additionally
 * surfaces a `[← Back to wiki page]` affordance. After the final
 * question a summary shows the score and a pass/fail outcome computed
 * with `app.quiz.pass_threshold`.
 */
import { useMemo, useState } from "react"

import { Button } from "@/components/ui/button"
import { StatusPill } from "@/components/ui/StatusPill"
import {
  isPassing,
  parseQuiz,
  type ChoiceKey,
} from "@/lib/quiz-parse"
import type { QuizAnswer, QuizProgress, WikiPageMeta } from "@/lib/ipc"
import { useT } from "@/i18n/useT"
import { InlineMarkdownText } from "./ExplanationText"

const CHOICE_KEYS: ChoiceKey[] = ["A", "B", "C", "D"]

interface QuizAnsweringProps {
  quizMd: string
  passThreshold: number
  /**
   * quiz-attempt-progress (design D6): wiki page index (keyed by slug)
   * for resolving the explanation's `[[slug]]` citations, and the
   * navigate handler invoked when a resolvable citation is activated.
   * Replaces the removed `[← Back to wiki page]` button.
   */
  pages?: Record<string, WikiPageMeta>
  onOpenWikiPage?: (slug: string) => void
  /**
   * quiz-attempt-progress (design D3): the attempt's saved sidecar state.
   * When present with prior `answers`, answering resumes at the first
   * question whose 1-based number is absent from them.
   */
  initialProgress?: QuizProgress | null
  /**
   * Called on every submission with the full updated [`QuizProgress`]
   * (status `in_progress`, or `completed` on the final question). The
   * caller persists it via `write_quiz_progress` — answering never spawns.
   */
  onPersist?: (progress: QuizProgress) => void
  /**
   * Phase 5.4 quiz-fullscreen-wizard-view marker: signals that this
   * component is hosted inside the wizard chrome (back-to-history
   * supplied by the wizard TabContentHeader). QuizAnswering currently
   * has no back-to-history button of its own (removed by
   * quiz-attempt-progress D6), so the prop is accepted as a marker
   * without altering rendering; it future-proofs the contract for any
   * additional chrome-supplied affordance.
   */
  embedded?: boolean
}

/**
 * Legacy resume fallback (design D3 interim): restore the LAST answered
 * question (highest `q` in `answers`) in its submitted state. Used only
 * when the sidecar has no `cursor` (legacy / prior-build attempts).
 */
function lastAnsweredIndex(answers: QuizAnswer[], total: number): number {
  if (answers.length === 0) return 0
  const maxQ = Math.max(...answers.map((a) => a.q))
  return Math.min(Math.max(maxQ, 1), total) - 1
}

/**
 * Resolve the resume position (design D3 final). A `cursor` restores the
 * exact spot (`q`/`revealed`); without one, fall back to last-answered.
 */
function resolveResume(
  initialProgress: QuizProgress | null | undefined,
  seededAnswers: QuizAnswer[],
  total: number,
): { idx: number; revealed: boolean } {
  const cursor = initialProgress?.cursor
  if (cursor) {
    const idx = Math.min(Math.max(cursor.q, 1), total) - 1
    return { idx, revealed: cursor.revealed }
  }
  const idx = lastAnsweredIndex(seededAnswers, total)
  return { idx, revealed: seededAnswers.some((a) => a.q === idx + 1) }
}

export function QuizAnswering({
  quizMd,
  passThreshold,
  pages,
  onOpenWikiPage,
  initialProgress,
  onPersist,
  // Phase 5.4: accept the `embedded` marker without destructuring it
  // into a binding — there is no body-level behavior change yet.
  embedded: _embedded,
}: QuizAnsweringProps) {
  const t = useT()
  const questions = useMemo(() => parseQuiz(quizMd), [quizMd])
  const seededAnswers = useMemo<QuizAnswer[]>(
    () => initialProgress?.answers ?? [],
    [initialProgress],
  )
  const [answers, setAnswers] = useState<QuizAnswer[]>(seededAnswers)
  // Resume to the exact cursor position (design D3 final); a sidecar
  // without a cursor falls back to last-answered-revealed. A fresh
  // attempt (no answers, no cursor) starts at Q1 blank.
  const resume = resolveResume(
    initialProgress,
    seededAnswers,
    questions.length || 1,
  )
  const restoredAnswer = resume.revealed
    ? seededAnswers.find((a) => a.q === resume.idx + 1)
    : undefined
  const [idx, setIdx] = useState(resume.idx)
  const [selected, setSelected] = useState<ChoiceKey | null>(
    restoredAnswer ? restoredAnswer.selected : null,
  )
  const [revealed, setRevealed] = useState(resume.revealed)
  const [correctCount, setCorrectCount] = useState(
    () => seededAnswers.filter((a) => a.correct).length,
  )
  const [startedAt] = useState(
    () => initialProgress?.started_at ?? new Date().toISOString(),
  )
  const [done, setDone] = useState(false)

  if (questions.length === 0) {
    return (
      <div data-testid="quiz-answering-empty" className="p-6 text-fg-secondary">
        {t("workspace.quiz.answering.parseEmpty")}
      </div>
    )
  }

  if (done) {
    const total = questions.length
    const pass = isPassing(correctCount, total, passThreshold)
    return (
      <div data-testid="quiz-summary" className="flex flex-col gap-3 p-6">
        <h3 className="text-body-lg font-medium text-fg-primary">
          {t("workspace.quiz.answering.summaryHeading")}
        </h3>
        <p data-testid="quiz-score" className="text-body-lg">
          {t("workspace.quiz.answering.scoreLine", {
            correct: correctCount,
            total,
            percent: Math.round((correctCount / total) * 100),
          })}
        </p>
        <p
          data-testid="quiz-outcome"
          className="flex items-center gap-2"
        >
          <StatusPill
            status={pass ? "done" : "failed"}
            variant="pill"
          />
          <span>
            {pass
              ? t("workspace.quiz.answering.outcomePassed", { n: passThreshold })
              : t("workspace.quiz.answering.outcomeFailed", { n: passThreshold })}
          </span>
        </p>
      </div>
    )
  }

  const q = questions[idx]
  const isCorrect = revealed && selected === q.answer

  function onSubmit() {
    if (selected == null) return
    setRevealed(true)
    const correct = selected === q.answer
    if (correct) setCorrectCount((c) => c + 1)

    // design D3: persist this submission to the attempt sidecar. The
    // final question's submission marks the attempt completed. Recompute
    // `answers` (replace any prior entry for this q — resume safety),
    // never store derived counts (single source of truth — design D1).
    const qNum = idx + 1
    const nextAnswers: QuizAnswer[] = [
      ...answers.filter((a) => a.q !== qNum),
      { q: qNum, selected, correct },
    ].sort((a, b) => a.q - b.q)
    setAnswers(nextAnswers)
    const isFinal = idx + 1 >= questions.length
    // design D3 final: cursor records the just-submitted question as
    // revealed, so reopening restores exactly this submitted screen.
    onPersist?.({
      schema_version: initialProgress?.schema_version ?? 1,
      answers: nextAnswers,
      status: isFinal ? "completed" : "in_progress",
      started_at: startedAt,
      completed_at: isFinal ? new Date().toISOString() : null,
      cursor: { q: qNum, revealed: true },
    })
  }

  function onNext() {
    if (idx + 1 >= questions.length) {
      setDone(true)
      return
    }
    const nextIdx = idx + 1
    setIdx(nextIdx)
    setSelected(null)
    setRevealed(false)
    // design D3 final: persist the advanced cursor (answers unchanged)
    // so reopening lands on the blank next question, not the previous
    // already-answered one.
    onPersist?.({
      schema_version: initialProgress?.schema_version ?? 1,
      answers,
      status: "in_progress",
      started_at: startedAt,
      completed_at: null,
      cursor: { q: nextIdx + 1, revealed: false },
    })
  }

  return (
    <div data-testid="quiz-answering" className="flex flex-col gap-4 p-6">
      <p className="text-body text-fg-secondary">
        {t("workspace.quiz.answering.questionCounter", {
          n: idx + 1,
          total: questions.length,
        })}
      </p>
      <h3 data-testid="quiz-stem" className="text-body-lg text-fg-primary">
        <InlineMarkdownText
          text={q.stem}
          pages={pages ?? {}}
          onOpenWikiPage={onOpenWikiPage}
        />
      </h3>
      <ul className="flex flex-col gap-2">
        {CHOICE_KEYS.map((k) => {
          const isAnswer = k === q.answer
          const isPicked = k === selected
          return (
            <li key={k}>
              <button
                type="button"
                data-testid={`quiz-choice-${k}`}
                disabled={revealed}
                onClick={() => setSelected(k)}
                className={[
                  "w-full rounded border px-3 py-2 text-left text-body-lg",
                  isPicked ? "border-accent" : "border-border",
                  revealed && isAnswer ? "bg-green-500/15" : "",
                  revealed && isPicked && !isAnswer ? "bg-red-500/15" : "",
                ].join(" ")}
              >
                {k}){" "}
                <InlineMarkdownText
                  text={q.choices[k]}
                  pages={pages ?? {}}
                  onOpenWikiPage={onOpenWikiPage}
                />
              </button>
            </li>
          )
        })}
      </ul>

      {!revealed && (
        <div>
          <Button
            data-testid="quiz-submit"
            onClick={onSubmit}
            disabled={selected == null}
          >
            {t("workspace.quiz.answering.submitButton")}
          </Button>
        </div>
      )}

      {revealed && (
        <div data-testid="quiz-reveal" className="flex flex-col gap-2">
          <p
            data-testid="quiz-verdict"
            className={isCorrect ? "text-green-500" : "text-red-500"}
          >
            {isCorrect
              ? t("workspace.quiz.answering.verdictCorrect")
              : t("workspace.quiz.answering.verdictIncorrect")}
          </p>
          <p data-testid="quiz-explanation" className="text-body-lg">
            <InlineMarkdownText
              text={q.explanation}
              pages={pages ?? {}}
              onOpenWikiPage={onOpenWikiPage}
            />
          </p>
          <div>
            <Button data-testid="quiz-next" onClick={onNext}>
              {idx + 1 >= questions.length
                ? t("workspace.quiz.answering.finishButton")
                : t("workspace.quiz.answering.nextButton")}
            </Button>
          </div>
        </div>
      )}
    </div>
  )
}
