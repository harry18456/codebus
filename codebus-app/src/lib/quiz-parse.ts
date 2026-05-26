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

import type { TFunction } from "@/i18n/useT"

export type ChoiceKey = "A" | "B" | "C" | "D"

export interface QuizQuestion {
  stem: string
  choices: Record<ChoiceKey, string>
  answer: ChoiceKey
  explanation: string
  /**
   * The `[[slug]]` wiki citations found in this question's explanation,
   * in first-seen order and de-duplicated (design D6). The answering and
   * review views render these as navigable wikilinks.
   */
  sources: string[]
}

/** Ordered, de-duplicated `[[slug]]` citations in an explanation. */
function extractSources(explanation: string): string[] {
  const out: string[] = []
  const seen = new Set<string>()
  for (const m of explanation.matchAll(/\[\[([^\]]+)\]\]/g)) {
    const slug = m[1].trim()
    if (slug && !seen.has(slug)) {
      seen.add(slug)
      out.push(slug)
    }
  }
  return out
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
      sources: extractSources(explanation),
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

/**
 * Derived per-attempt history badge (quiz-attempt-progress design D4).
 * `total` is the question count parsed from the attempt markdown; the
 * answered/correct/score values are recomputed from the sidecar answers
 * (never stored — single source of truth, design D1):
 *
 *   not-started        → `0/N`
 *   in-progress        → `X/N`
 *   completed          → `X/N · S% · pass|fail`
 */
export function quizBadge(
  status: "not_started" | "in_progress" | "completed",
  answered: number,
  correct: number,
  total: number,
  passThresholdPercent: number,
  t: TFunction,
): string {
  if (status === "not_started") return `0/${total}`
  if (status === "in_progress") return `${answered}/${total}`
  const score = total === 0 ? 0 : Math.round((correct / total) * 100)
  const verdict = isPassing(correct, total, passThresholdPercent)
    ? t("quiz.badge.pass")
    : t("quiz.badge.fail")
  return `${answered}/${total} · ${score}% · ${verdict}`
}
