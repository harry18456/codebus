import { describe, expect, it } from "vitest"
import { renderHook } from "@testing-library/react"

import { useT } from "@/i18n/useT"

import { parseQuiz, quizBadge } from "./quiz-parse"

// quiz-attempt-progress task 6.1 (design D6): each question exposes the
// `[[slug]]` citations found in its `## Explanation`, in order and
// de-duplicated, so the answering/review explanation can render them as
// navigable wikilinks (replacing the back-to-wiki button).

const MD = `## Q1. What does AuthMiddleware return on an expired token?

- A) 200
- B) 301
- C) 401
- D) 500

## Answer: C

## Explanation: Per [[auth-middleware-verification]] all auth failures are 401; see also [[login-token-minting]] and again [[auth-middleware-verification]] for the lookup step.

## Q2. Which claim carries the user id?

- A) sub
- B) aud
- C) iss
- D) jti

## Answer: A

## Explanation: The sub claim identifies the subject. (no citation here)`

describe("parseQuiz sources", () => {
  it("extracts per-question explanation [[slug]] citations, ordered and de-duplicated", () => {
    const qs = parseQuiz(MD)
    expect(qs).toHaveLength(2)
    expect(qs[0].sources).toEqual([
      "auth-middleware-verification",
      "login-token-minting",
    ])
  })

  it("yields an empty sources array when the explanation has no citation", () => {
    const qs = parseQuiz(MD)
    expect(qs[1].sources).toEqual([])
  })
})

describe("quizBadge i18n verdict", () => {
  it("renders 'pass' verdict in en locale when score >= threshold", () => {
    const { result } = renderHook(() => useT("en"))
    expect(quizBadge("completed", 4, 4, 4, 75, result.current)).toBe(
      "4/4 · 100% · pass",
    )
  })

  it("renders '通過' verdict in zh locale when score >= threshold", () => {
    const { result } = renderHook(() => useT("zh"))
    expect(quizBadge("completed", 4, 4, 4, 75, result.current)).toBe(
      "4/4 · 100% · 通過",
    )
  })

  it("renders 'fail' verdict in en locale when score < threshold", () => {
    const { result } = renderHook(() => useT("en"))
    expect(quizBadge("completed", 4, 1, 4, 75, result.current)).toBe(
      "4/4 · 25% · fail",
    )
  })

  it("renders '未通過' verdict in zh locale when score < threshold", () => {
    const { result } = renderHook(() => useT("zh"))
    expect(quizBadge("completed", 4, 1, 4, 75, result.current)).toBe(
      "4/4 · 25% · 未通過",
    )
  })
})
