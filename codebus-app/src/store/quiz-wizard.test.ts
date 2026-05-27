import { beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@/lib/ipc", () => ({
  cancelQuiz: vi.fn().mockResolvedValue(undefined),
}))

import { cancelQuiz } from "@/lib/ipc"
import { useQuizWizardStore } from "./quiz-wizard"

const BUCKETS = {
  concepts: ["c1"],
  entities: [],
  modules: ["m1", "m2"],
  processes: [],
  synthesis: ["s1"],
}

const RESULT = { score: 4, total: 5, wrong: [2] }

describe("quiz-wizard store", () => {
  beforeEach(() => {
    useQuizWizardStore.setState({
      step: { kind: "topic" },
      currentRunId: null,
    })
    vi.mocked(cancelQuiz).mockClear()
  })

  describe("state machine transitions", () => {
    it("starts in topic step", () => {
      expect(useQuizWizardStore.getState().step.kind).toBe("topic")
    })

    it("topic -> scope_confirm with staged id and buckets", () => {
      useQuizWizardStore.getState().goToScopeConfirm("stage-1", BUCKETS)
      const step = useQuizWizardStore.getState().step
      expect(step.kind).toBe("scope_confirm")
      if (step.kind === "scope_confirm") {
        expect(step.stagedId).toBe("stage-1")
        expect(step.buckets).toEqual(BUCKETS)
      }
    })

    it("scope_confirm -> generating preserves staged id", () => {
      useQuizWizardStore.getState().goToScopeConfirm("stage-1", BUCKETS)
      useQuizWizardStore.getState().goToGenerating("stage-1", "run-1")
      const step = useQuizWizardStore.getState().step
      expect(step.kind).toBe("generating")
      if (step.kind === "generating") {
        expect(step.stagedId).toBe("stage-1")
      }
      expect(useQuizWizardStore.getState().currentRunId).toBe("run-1")
    })

    it("generating -> review_pending preserves staged id", () => {
      useQuizWizardStore.getState().goToGenerating("stage-1", "run-1")
      useQuizWizardStore.getState().goToReviewPending("stage-1")
      const step = useQuizWizardStore.getState().step
      expect(step.kind).toBe("review_pending")
      if (step.kind === "review_pending") {
        expect(step.stagedId).toBe("stage-1")
      }
    })

    it("review_pending -> reviewing preserves staged id", () => {
      useQuizWizardStore.getState().goToReviewPending("stage-1")
      useQuizWizardStore.getState().goToReviewing("stage-1")
      const step = useQuizWizardStore.getState().step
      expect(step.kind).toBe("reviewing")
      if (step.kind === "reviewing") {
        expect(step.stagedId).toBe("stage-1")
      }
    })

    it("reviewing -> completion carries result", () => {
      useQuizWizardStore.getState().goToReviewing("stage-1")
      useQuizWizardStore.getState().goToCompletion("stage-1", RESULT)
      const step = useQuizWizardStore.getState().step
      expect(step.kind).toBe("completion")
      if (step.kind === "completion") {
        expect(step.stagedId).toBe("stage-1")
        expect(step.result).toEqual(RESULT)
      }
    })

    it("goToTopic resets to topic and clears runId", () => {
      useQuizWizardStore.getState().goToGenerating("stage-1", "run-1")
      useQuizWizardStore.getState().goToTopic()
      expect(useQuizWizardStore.getState().step.kind).toBe("topic")
      expect(useQuizWizardStore.getState().currentRunId).toBeNull()
    })
  })

  describe("cancel cleanup", () => {
    it("cancel from scope_confirm resets to topic and does not call cancelQuiz when no runId", async () => {
      useQuizWizardStore.getState().goToScopeConfirm("stage-1", BUCKETS)
      await useQuizWizardStore.getState().cancel()
      expect(useQuizWizardStore.getState().step.kind).toBe("topic")
      expect(useQuizWizardStore.getState().currentRunId).toBeNull()
      expect(cancelQuiz).not.toHaveBeenCalled()
    })

    it("cancel from generating invokes cancelQuiz with currentRunId then resets to topic", async () => {
      useQuizWizardStore.getState().goToGenerating("stage-1", "run-7")
      await useQuizWizardStore.getState().cancel()
      expect(cancelQuiz).toHaveBeenCalledWith("run-7")
      expect(useQuizWizardStore.getState().step.kind).toBe("topic")
      expect(useQuizWizardStore.getState().currentRunId).toBeNull()
    })

    it("cancel from review_pending resets to topic; cancelQuiz called only if runId still tracked", async () => {
      useQuizWizardStore.getState().goToGenerating("stage-1", "run-7")
      useQuizWizardStore.getState().goToReviewPending("stage-1")
      await useQuizWizardStore.getState().cancel()
      expect(useQuizWizardStore.getState().step.kind).toBe("topic")
      expect(cancelQuiz).toHaveBeenCalledWith("run-7")
    })

    it("cancel still completes frontend cleanup when cancelQuiz rejects", async () => {
      vi.mocked(cancelQuiz).mockRejectedValueOnce(new Error("boom"))
      const errSpy = vi.spyOn(console, "error").mockImplementation(() => {})
      useQuizWizardStore.getState().goToGenerating("stage-1", "run-7")
      await useQuizWizardStore.getState().cancel()
      expect(useQuizWizardStore.getState().step.kind).toBe("topic")
      expect(useQuizWizardStore.getState().currentRunId).toBeNull()
      expect(errSpy).toHaveBeenCalled()
      errSpy.mockRestore()
    })
  })

  // Per spec app-workspace § Quiz Wizard Cancel Cleanup — explicit
  // coverage of the four spec scenarios on top of the state-machine
  // cancel cases above. URL clearing and TabContentHeader restoration
  // are integration concerns (QuizTab.test.tsx); these test the
  // store-level contract that survives any caller.
  describe("§ Quiz Wizard Cancel Cleanup scenarios", () => {
    it("scenario: Cancel from scope_confirm returns to history (store reset)", async () => {
      useQuizWizardStore.getState().goToScopeConfirm("stage-z", BUCKETS)
      await useQuizWizardStore.getState().cancel()
      expect(useQuizWizardStore.getState().step.kind).toBe("topic")
      expect(useQuizWizardStore.getState().currentRunId).toBeNull()
    })

    it("scenario: Cancel from generating invokes cancelQuiz with the tracked runId", async () => {
      useQuizWizardStore.getState().goToGenerating("stage-z", "run-z")
      await useQuizWizardStore.getState().cancel()
      expect(cancelQuiz).toHaveBeenCalledWith("run-z")
    })

    it("scenario: cancelQuiz rejection does not block the frontend store cleanup", async () => {
      vi.mocked(cancelQuiz).mockRejectedValueOnce(new Error("backend down"))
      const errSpy = vi.spyOn(console, "error").mockImplementation(() => {})
      useQuizWizardStore.getState().goToGenerating("stage-z", "run-z")
      await useQuizWizardStore.getState().cancel()
      expect(useQuizWizardStore.getState().step.kind).toBe("topic")
      expect(errSpy).toHaveBeenCalled()
      errSpy.mockRestore()
    })

    it("scenario: staged wizard state is in-memory only (no disk write call surface)", () => {
      // Cancel completes synchronously after reset; the store API exposes
      // no disk-write fn. This test pins the absence — any future
      // staged-persistence addition surfaces as a missing method.
      const apiKeys = Object.keys(useQuizWizardStore.getState())
      expect(apiKeys).not.toContain("persistStaged")
      expect(apiKeys).not.toContain("writeStaged")
    })
  })

  describe("hydrateFromUrl", () => {
    it("hydrates scope_confirm when staged record exists in store", () => {
      useQuizWizardStore.setState({
        step: { kind: "scope_confirm", stagedId: "stage-2", buckets: BUCKETS },
        currentRunId: null,
      })
      const params = new URLSearchParams("?quiz_step=scope_confirm&staged_id=stage-2")
      useQuizWizardStore.getState().hydrateFromUrl(params)
      const step = useQuizWizardStore.getState().step
      expect(step.kind).toBe("scope_confirm")
      if (step.kind === "scope_confirm") {
        expect(step.stagedId).toBe("stage-2")
      }
    })

    it("falls back to topic step and warns when staged_id is missing in store", () => {
      const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {})
      const params = new URLSearchParams("?quiz_step=reviewing&staged_id=nope")
      useQuizWizardStore.getState().hydrateFromUrl(params)
      expect(useQuizWizardStore.getState().step.kind).toBe("topic")
      expect(warnSpy).toHaveBeenCalled()
      const msg = warnSpy.mock.calls[0]?.join(" ") ?? ""
      expect(msg).toContain("nope")
      warnSpy.mockRestore()
    })

    it("leaves step at topic when no quiz_step param is present", () => {
      const params = new URLSearchParams("")
      useQuizWizardStore.getState().hydrateFromUrl(params)
      expect(useQuizWizardStore.getState().step.kind).toBe("topic")
    })
  })
})
