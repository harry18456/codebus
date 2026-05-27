/**
 * Wizard URL state hook — see app-workspace § Quiz Wizard URL State
 * Persistence. Owns exactly the two query params `quiz_step` and
 * `staged_id`; other params are preserved unchanged. Mount / unmount
 * SHALL NOT push history entries — only explicit `write` calls do.
 */
export interface QuizUrlState {
  quiz_step: string | null
  staged_id: string | null
}

const OWNED_KEYS = ["quiz_step", "staged_id"] as const

function buildSearch(state: QuizUrlState): string {
  const params = new URLSearchParams(window.location.search)
  for (const key of OWNED_KEYS) {
    const value = state[key]
    if (value === null) {
      params.delete(key)
    } else {
      params.set(key, value)
    }
  }
  const next = params.toString()
  return next.length === 0 ? "" : `?${next}`
}

export function useUrlState(): {
  read: () => QuizUrlState
  write: (state: QuizUrlState) => void
} {
  function read(): QuizUrlState {
    const params = new URLSearchParams(window.location.search)
    return {
      quiz_step: params.get("quiz_step"),
      staged_id: params.get("staged_id"),
    }
  }

  function write(state: QuizUrlState): void {
    const search = buildSearch(state)
    const url = `${window.location.pathname}${search}${window.location.hash}`
    window.history.pushState(null, "", url)
  }

  return { read, write }
}
