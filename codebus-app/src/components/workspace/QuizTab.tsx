/**
 * Quiz tab — plan-confirm-generate flow (v3-app-quiz tasks 5.1 + 5.2).
 *
 * Spec: app-workspace § Quiz Tab Plan-Confirm-Generate Flow + § Tauri
 * IPC Commands for Quiz Plan and Generate Lifecycle, design D1.
 *
 * State machine:
 *   idle ──Start──▶ planning ──quiz-plan-terminal──▶ confirm | no_match | error
 *   confirm ──[改]──▶ idle
 *   confirm ──[確認]──▶ generating ──quiz-generate-terminal──▶ ready | error
 *
 * The confirm gate is load-bearing (design D1): the generate spawn is a
 * SEPARATE IPC call (`spawnQuizGenerate`) issued ONLY on explicit user
 * confirmation — `spawnQuizPlan` never starts generation. A no-match
 * plan shows the reason and writes no file (no generate spawn).
 *
 * The answering view (task 5.4) and history list (task 5.5) build on
 * the `ready` phase (which holds `quizMd`) and the history region; this
 * task delivers the flow + confirm gate.
 */
import { useEffect, useRef, useState } from "react"

import { listen, type UnlistenFn } from "@tauri-apps/api/event"

import { Button } from "@/components/ui/button"
import { QuizAnswering } from "./QuizAnswering"
import {
  spawnQuizGenerate,
  spawnQuizPlan,
  listQuizAttempts,
  readQuizAttempt,
  type QuizAttemptMeta,
  type QuizGenerateTerminalPayload,
  type QuizPlanTerminalPayload,
  type QuizTriggerArg,
} from "@/lib/ipc"

type Phase =
  | "idle"
  | "planning"
  | "confirm"
  | "generating"
  | "ready"
  | "no_match"
  | "error"
  | "attempt"

interface QuizTabProps {
  vaultPath: string
  /**
   * task 5.3 — set by Workspace when the user clicks `[Quiz me on
   * this]` on a wiki content page. Triggers the Page flow: skip
   * planning, generate directly with `pages=[pendingPage]`. Cleared
   * via `onPendingConsumed` once consumed.
   */
  pendingPage?: string | null
  onPendingConsumed?: () => void
}

// task 5.2 uses the default question count; wiring it to the shared
// `quiz.default_length` config is part of the settings/answering scope.
const DEFAULT_QUESTION_COUNT = 5
// task 5.4 default; wiring to `app.quiz.pass_threshold` (settings store)
// is a follow-up — the spec pass-threshold scenario is verified in
// QuizAnswering.test via the prop.
const DEFAULT_PASS_THRESHOLD = 80

/** Group history attempts by slug, preserving the (newest-first) order. */
function groupBySlug(
  attempts: QuizAttemptMeta[],
): [string, QuizAttemptMeta[]][] {
  const map = new Map<string, QuizAttemptMeta[]>()
  for (const a of attempts) {
    const arr = map.get(a.slug) ?? []
    arr.push(a)
    map.set(a.slug, arr)
  }
  return [...map.entries()]
}

export function QuizTab({
  vaultPath,
  pendingPage,
  onPendingConsumed,
}: QuizTabProps) {
  const [phase, setPhase] = useState<Phase>("idle")
  const [topic, setTopic] = useState("")
  const [pages, setPages] = useState<string[]>([])
  const [reason, setReason] = useState("")
  const [errorMsg, setErrorMsg] = useState("")
  const [quizMd, setQuizMd] = useState("")
  const [attempts, setAttempts] = useState<QuizAttemptMeta[]>([])
  const [attemptMd, setAttemptMd] = useState("")
  const [viewLog, setViewLog] = useState<string | null>(null)
  const unlistenRef = useRef<UnlistenFn | null>(null)

  async function openAttempt(a: QuizAttemptMeta) {
    try {
      const md = await readQuizAttempt(vaultPath, a.path)
      setAttemptMd(md)
      setPhase("attempt")
    } catch (e) {
      setErrorMsg(String(e))
      setPhase("error")
    }
  }

  // task 5.5 — refresh the history list whenever the tab is idle (mount,
  // back from an attempt, or after a generated quiz returns to idle).
  useEffect(() => {
    if (phase !== "idle") return
    let alive = true
    void listQuizAttempts(vaultPath)
      .then((a) => {
        if (alive) setAttempts(a)
      })
      .catch(() => {
        /* missing dir / read error → leave list empty */
      })
    return () => {
      alive = false
    }
  }, [phase, vaultPath])

  function clearListener() {
    if (unlistenRef.current) {
      unlistenRef.current()
      unlistenRef.current = null
    }
  }

  // task 5.3 Page flow: a content page's [Quiz me on this] sets
  // `pendingPage`. Skip planning entirely and generate directly with
  // pages=[pendingPage]. Guarded on idle so a re-render mid-flow does
  // not re-trigger; consumed immediately so the same page can be
  // re-quizzed later.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (pendingPage && phase === "idle") {
      onPendingConsumed?.()
      void startGenerate([pendingPage], {
        kind: "wiki_preview",
        target_page: pendingPage,
      })
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pendingPage])

  async function onStart() {
    if (!topic.trim()) return
    setPhase("planning")
    const handle = await listen<QuizPlanTerminalPayload>(
      "quiz-plan-terminal",
      (event) => {
        clearListener()
        const r = event.payload.result
        if (r.kind === "scope") {
          setPages(r.pages)
          setPhase("confirm")
        } else if (r.kind === "no_match") {
          setReason(r.reason)
          setPhase("no_match")
        } else if (r.kind === "failed") {
          setErrorMsg(r.message)
          setPhase("error")
        } else {
          setPhase("idle")
        }
      },
    )
    unlistenRef.current = handle
    try {
      await spawnQuizPlan(vaultPath, topic.trim())
    } catch (e) {
      clearListener()
      setErrorMsg(String(e))
      setPhase("error")
    }
  }

  /**
   * Shared generate path. Used by the Goal-flow confirm button
   * (`onConfirm`) AND the Page flow (`[Quiz me on this]` via the
   * `pendingPage` prop) — the Page flow skips planning entirely
   * (design D1 / spec: "Quiz-me-on-this skips planning").
   */
  async function startGenerate(
    genPages: string[],
    trigger: QuizTriggerArg,
  ) {
    setPages(genPages)
    setPhase("generating")
    const handle = await listen<QuizGenerateTerminalPayload>(
      "quiz-generate-terminal",
      (event) => {
        clearListener()
        const r = event.payload.result
        if (r.kind === "succeeded") {
          setQuizMd(r.quiz_md)
          setPhase("ready")
        } else if (r.kind === "failed") {
          setErrorMsg(r.message)
          setPhase("error")
        } else {
          setPhase("idle")
        }
      },
    )
    unlistenRef.current = handle
    try {
      await spawnQuizGenerate(
        vaultPath,
        genPages,
        DEFAULT_QUESTION_COUNT,
        trigger,
      )
    } catch (e) {
      clearListener()
      setErrorMsg(String(e))
      setPhase("error")
    }
  }

  function onConfirm() {
    void startGenerate(pages, { kind: "ai_planned", topic })
  }

  function reset() {
    clearListener()
    setPhase("idle")
    setPages([])
    setReason("")
    setErrorMsg("")
    setQuizMd("")
  }

  return (
    <div
      data-testid="quiz-tab"
      className="flex h-full w-full flex-col px-8 py-6"
    >
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-[15px] font-medium text-fg-primary">
          Quiz history
        </h2>
        <Button
          data-testid="new-quiz"
          onClick={() => setPhase("idle")}
          disabled={phase !== "idle" && phase !== "ready"}
        >
          + New quiz
        </Button>
      </div>

      {phase === "idle" && (
        <div data-testid="quiz-idle" className="flex flex-col gap-3">
          <input
            data-testid="quiz-topic-input"
            className="rounded border border-border bg-bg-secondary px-3 py-2 text-[14px]"
            placeholder="What do you want to be quizzed on?"
            value={topic}
            onChange={(e) => setTopic(e.target.value)}
          />
          <div>
            <Button
              data-testid="quiz-start"
              onClick={onStart}
              disabled={!topic.trim()}
            >
              Start
            </Button>
          </div>
          <div
            data-testid="quiz-history"
            className="mt-4 flex-1 overflow-auto"
          >
            {attempts.length === 0 ? (
              <p className="text-[14px] text-fg-secondary">
                No quizzes yet — start one above
              </p>
            ) : (
              groupBySlug(attempts).map(([slug, rows]) => (
                <div
                  key={slug}
                  data-testid="quiz-history-group"
                  className="mb-3"
                >
                  <p className="text-[13px] font-medium text-fg-primary">
                    {slug}
                  </p>
                  {rows.map((a) => (
                    <div
                      key={a.path}
                      data-testid="quiz-attempt-row"
                      className="flex items-center gap-2 py-1 text-[13px]"
                    >
                      <button
                        type="button"
                        data-testid="quiz-attempt-open"
                        className="flex-1 text-left text-fg-secondary hover:text-fg-primary"
                        onClick={() => void openAttempt(a)}
                      >
                        {a.topic ?? a.target_page ?? a.slug} · {a.quiz_id}
                      </button>
                      {a.events_log && (
                        <button
                          type="button"
                          data-testid="quiz-view-log"
                          title={a.events_log}
                          className="text-fg-secondary hover:text-fg-primary"
                          onClick={() => setViewLog(a.events_log)}
                        >
                          view log
                        </button>
                      )}
                    </div>
                  ))}
                </div>
              ))
            )}
            {viewLog && (
              <p
                data-testid="quiz-view-log-path"
                className="mt-2 text-[12px] text-fg-secondary"
              >
                Generation log: {viewLog}
              </p>
            )}
          </div>
        </div>
      )}

      {phase === "attempt" && (
        <div
          data-testid="quiz-attempt-view"
          className="flex flex-1 flex-col gap-2"
        >
          <div>
            <Button
              data-testid="quiz-attempt-back"
              onClick={() => {
                setAttemptMd("")
                setPhase("idle")
              }}
            >
              ← Back to history
            </Button>
          </div>
          <pre className="flex-1 overflow-auto rounded bg-bg-secondary p-3 text-[12px]">
            {attemptMd}
          </pre>
        </div>
      )}

      {phase === "planning" && (
        <p data-testid="quiz-planning" className="text-[14px] text-fg-secondary">
          Planning quiz scope…
        </p>
      )}

      {phase === "confirm" && (
        <div data-testid="quiz-confirm" className="flex flex-col gap-3">
          <p className="text-[14px] text-fg-primary">
            Planned scope — confirm to generate the quiz:
          </p>
          <ul className="text-[13px] text-fg-secondary">
            {pages.map((p) => (
              <li key={p} data-testid="quiz-scope-page">
                {p}
              </li>
            ))}
          </ul>
          <div className="flex gap-2">
            <Button data-testid="quiz-revise" onClick={reset}>
              改
            </Button>
            <Button data-testid="quiz-generate" onClick={onConfirm}>
              確認
            </Button>
          </div>
        </div>
      )}

      {phase === "generating" && (
        <p
          data-testid="quiz-generating"
          className="text-[14px] text-fg-secondary"
        >
          Generating questions…
        </p>
      )}

      {phase === "ready" && (
        <div data-testid="quiz-ready" className="flex flex-1 flex-col">
          <QuizAnswering
            quizMd={quizMd}
            passThreshold={DEFAULT_PASS_THRESHOLD}
          />
        </div>
      )}

      {phase === "no_match" && (
        <div data-testid="quiz-no-match" className="flex flex-col gap-3">
          <p className="text-[14px] text-fg-primary">
            No matching wiki pages: {reason}
          </p>
          <div>
            <Button data-testid="quiz-back" onClick={reset}>
              Back
            </Button>
          </div>
        </div>
      )}

      {phase === "error" && (
        <div data-testid="quiz-error" className="flex flex-col gap-3">
          <p className="text-[14px] text-fg-primary">Quiz failed: {errorMsg}</p>
          <div>
            <Button data-testid="quiz-back" onClick={reset}>
              Back
            </Button>
          </div>
        </div>
      )}
    </div>
  )
}
