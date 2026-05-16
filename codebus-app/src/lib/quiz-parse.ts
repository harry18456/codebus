/**
 * Parser for the quiz markdown body emitted by the generate spawn
 * (v3-app-quiz task 5.4). The body has NO frontmatter (the agent emits
 * only the question body per design D4; caller frontmatter is injected
 * at persistence and is not present in `QuizReport.quiz_md`).
 *
 * Expected shape (codebus-quiz SKILL):
 *
 *   ## Q1. <stem>
 *
 *   - A) <choice>
 *   - B) <choice>
 *   - C) <choice>
 *   - D) <choice>
 *
 *   ## Answer: B
 *
 *   ## Explanation: <text, may cite [[slug]]>
 *
 *   ## Q2. ...
 *
 * Parsing is tolerant of blank lines and surrounding whitespace.
 */

export type ChoiceKey = "A" | "B" | "C" | "D"

export interface QuizQuestion {
  stem: string
  choices: Record<ChoiceKey, string>
  answer: ChoiceKey
  explanation: string
}

const CHOICE_KEYS: ChoiceKey[] = ["A", "B", "C", "D"]

function isChoiceKey(s: string): s is ChoiceKey {
  return (CHOICE_KEYS as string[]).includes(s)
}

/**
 * Parse the quiz body into questions. Malformed blocks (missing answer,
 * fewer than 4 choices, answer key not A–D) are skipped rather than
 * throwing — a partial quiz still lets the user answer the well-formed
 * questions instead of hard-failing the whole attempt.
 */
export function parseQuiz(md: string): QuizQuestion[] {
  const blocks = md
    .split(/^##\s+Q\d+\.\s*/m)
    .map((b) => b.trim())
    .filter((b) => b.length > 0)

  const questions: QuizQuestion[] = []
  for (const block of blocks) {
    const lines = block.split("\n")
    const stem = (lines[0] ?? "").trim()
    if (!stem) continue

    const choices: Partial<Record<ChoiceKey, string>> = {}
    let answer: ChoiceKey | null = null
    let explanation = ""

    for (const raw of lines.slice(1)) {
      const line = raw.trim()
      const choiceMatch = line.match(/^-\s*([A-D])\)\s*(.+)$/)
      if (choiceMatch) {
        const key = choiceMatch[1]
        if (isChoiceKey(key)) choices[key] = choiceMatch[2].trim()
        continue
      }
      const answerMatch = line.match(/^##\s+Answer:\s*([A-D])\b/)
      if (answerMatch && isChoiceKey(answerMatch[1])) {
        answer = answerMatch[1]
        continue
      }
      const explMatch = line.match(/^##\s+Explanation:\s*(.*)$/)
      if (explMatch) {
        explanation = explMatch[1].trim()
        continue
      }
    }

    const haveAllChoices = CHOICE_KEYS.every((k) => choices[k] != null)
    if (!haveAllChoices || answer == null) continue

    questions.push({
      stem,
      choices: choices as Record<ChoiceKey, string>,
      answer,
      explanation,
    })
  }
  return questions
}

/**
 * Pass/fail against `app.quiz.pass_threshold` (percent). Pass when the
 * score percentage is at or above the threshold — spec example: 4/5
 * (80%) with threshold 80 is a pass.
 */
export function isPassing(
  correct: number,
  total: number,
  thresholdPercent: number,
): boolean {
  if (total === 0) return false
  return (correct / total) * 100 >= thresholdPercent
}
