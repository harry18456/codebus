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
import {
  isPassing,
  parseQuiz,
  type ChoiceKey,
} from "@/lib/quiz-parse"

const CHOICE_KEYS: ChoiceKey[] = ["A", "B", "C", "D"]

interface QuizAnsweringProps {
  quizMd: string
  passThreshold: number
  /** task 5.4 — wiki-return affordance for an incorrect answer. */
  onBackToWiki?: () => void
}

export function QuizAnswering({
  quizMd,
  passThreshold,
  onBackToWiki,
}: QuizAnsweringProps) {
  const questions = useMemo(() => parseQuiz(quizMd), [quizMd])
  const [idx, setIdx] = useState(0)
  const [selected, setSelected] = useState<ChoiceKey | null>(null)
  const [revealed, setRevealed] = useState(false)
  const [correctCount, setCorrectCount] = useState(0)
  const [done, setDone] = useState(false)

  if (questions.length === 0) {
    return (
      <div data-testid="quiz-answering-empty" className="p-6 text-fg-secondary">
        Quiz could not be parsed — no well-formed questions.
      </div>
    )
  }

  if (done) {
    const total = questions.length
    const pass = isPassing(correctCount, total, passThreshold)
    return (
      <div data-testid="quiz-summary" className="flex flex-col gap-3 p-6">
        <h3 className="text-[15px] font-medium text-fg-primary">
          Quiz complete
        </h3>
        <p data-testid="quiz-score" className="text-[14px]">
          Score: {correctCount} / {total} (
          {Math.round((correctCount / total) * 100)}%)
        </p>
        <p
          data-testid="quiz-outcome"
          className={pass ? "text-green-500" : "text-red-500"}
        >
          {pass ? "Passed" : "Failed"} (threshold {passThreshold}%)
        </p>
      </div>
    )
  }

  const q = questions[idx]
  const isCorrect = revealed && selected === q.answer

  function onSubmit() {
    if (selected == null) return
    setRevealed(true)
    if (selected === q.answer) setCorrectCount((c) => c + 1)
  }

  function onNext() {
    if (idx + 1 >= questions.length) {
      setDone(true)
      return
    }
    setIdx((i) => i + 1)
    setSelected(null)
    setRevealed(false)
  }

  return (
    <div data-testid="quiz-answering" className="flex flex-col gap-4 p-6">
      <p className="text-[13px] text-fg-secondary">
        Question {idx + 1} of {questions.length}
      </p>
      <h3 data-testid="quiz-stem" className="text-[15px] text-fg-primary">
        {q.stem}
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
                  "w-full rounded border px-3 py-2 text-left text-[14px]",
                  isPicked ? "border-accent" : "border-border",
                  revealed && isAnswer ? "bg-green-500/15" : "",
                  revealed && isPicked && !isAnswer ? "bg-red-500/15" : "",
                ].join(" ")}
              >
                {k}) {q.choices[k]}
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
            Submit
          </Button>
        </div>
      )}

      {revealed && (
        <div data-testid="quiz-reveal" className="flex flex-col gap-2">
          <p
            data-testid="quiz-verdict"
            className={isCorrect ? "text-green-500" : "text-red-500"}
          >
            {isCorrect ? "Correct" : "Incorrect"}
          </p>
          <p data-testid="quiz-explanation" className="text-[14px]">
            {q.explanation}
          </p>
          {!isCorrect && (
            <div>
              <Button
                data-testid="quiz-back-to-wiki"
                onClick={() => onBackToWiki?.()}
              >
                ← Back to wiki page
              </Button>
            </div>
          )}
          <div>
            <Button data-testid="quiz-next" onClick={onNext}>
              {idx + 1 >= questions.length ? "Finish" : "Next"}
            </Button>
          </div>
        </div>
      )}
    </div>
  )
}
