import { create } from "zustand"

import { cancelQuiz } from "@/lib/ipc"

export const BUCKET_IDS = [
  "modules",
  "processes",
  "synthesis",
  "concepts",
  "entities",
] as const

export type BucketId = (typeof BUCKET_IDS)[number]

export type ScopeBuckets = Record<BucketId, string[]>

export interface QuizResult {
  score: number
  total: number
  wrong: number[]
}

export type QuizWizardStep =
  | { kind: "topic" }
  | { kind: "scope_confirm"; stagedId: string; buckets: ScopeBuckets }
  | { kind: "generating"; stagedId: string }
  | { kind: "review_pending"; stagedId: string }
  | { kind: "reviewing"; stagedId: string }
  | { kind: "completion"; stagedId: string; result: QuizResult }

export interface QuizWizardState {
  step: QuizWizardStep
  currentRunId: string | null
  goToTopic: () => void
  goToScopeConfirm: (stagedId: string, buckets: ScopeBuckets) => void
  goToGenerating: (stagedId: string, runId?: string) => void
  goToReviewPending: (stagedId: string) => void
  goToReviewing: (stagedId: string) => void
  goToCompletion: (stagedId: string, result: QuizResult) => void
  cancel: () => Promise<void>
  hydrateFromUrl: (params: URLSearchParams) => void
}

export function generateStagedId(): string {
  return crypto.randomUUID()
}

function stagedIdOfStep(step: QuizWizardStep): string | null {
  return step.kind === "topic" ? null : step.stagedId
}

export const useQuizWizardStore = create<QuizWizardState>((set, get) => ({
  step: { kind: "topic" },
  currentRunId: null,
  goToTopic() {
    set({ step: { kind: "topic" }, currentRunId: null })
  },
  goToScopeConfirm(stagedId, buckets) {
    set({ step: { kind: "scope_confirm", stagedId, buckets } })
  },
  goToGenerating(stagedId, runId) {
    set({
      step: { kind: "generating", stagedId },
      currentRunId: runId ?? get().currentRunId,
    })
  },
  goToReviewPending(stagedId) {
    set({ step: { kind: "review_pending", stagedId } })
  },
  goToReviewing(stagedId) {
    set({ step: { kind: "reviewing", stagedId } })
  },
  goToCompletion(stagedId, result) {
    set({ step: { kind: "completion", stagedId, result } })
  },
  async cancel() {
    const runId = get().currentRunId
    if (runId !== null) {
      try {
        await cancelQuiz(runId)
      } catch (err) {
        console.error("[quiz-wizard] cancelQuiz failed", err)
      }
    }
    set({ step: { kind: "topic" }, currentRunId: null })
  },
  hydrateFromUrl(params) {
    const quizStep = params.get("quiz_step")
    if (!quizStep) {
      return
    }
    const stagedIdParam = params.get("staged_id")
    const inStoreStagedId = stagedIdOfStep(get().step)
    if (stagedIdParam !== null && stagedIdParam !== inStoreStagedId) {
      console.warn(
        `[quiz-wizard] hydrateFromUrl: staged_id=${stagedIdParam} not present in store, falling back to topic`,
      )
      set({ step: { kind: "topic" }, currentRunId: null })
    }
  },
}))
