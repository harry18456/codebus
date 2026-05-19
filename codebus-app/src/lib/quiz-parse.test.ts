import { describe, expect, it } from "vitest"

import { parseQuiz } from "./quiz-parse"

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
