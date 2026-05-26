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
import { useSettingsStore } from "@/store/settings"
import { useT } from "@/i18n/useT"
import { useWatcherEvent } from "@/hooks/useWatcherEvent"
import { QuizAnswering } from "./QuizAnswering"
import { WatcherStatusBanner } from "./WatcherStatusBanner"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { QuizGenerationLog } from "./QuizGenerationLog"
import { QuizReview } from "./QuizReview"
import { parseQuiz, quizBadge } from "@/lib/quiz-parse"
import {
  ActivityStreamItem,
  ThoughtItem,
  foldTimeline,
} from "./ActivityStreamItem"
import {
  spawnQuizGenerate,
  spawnQuizPlan,
  listQuizAttempts,
  readQuizAttempt,
  readQuizProgress,
  writeQuizProgress,
  type QuizAttemptMeta,
  type QuizGenerateTerminalPayload,
  type QuizPlanTerminalPayload,
  type QuizProgress,
  type QuizStreamPayload,
  type QuizTriggerArg,
  type VerbEvent,
  type WikiPageMeta,
} from "@/lib/ipc"

/** Sidecar path for an attempt markdown (`<id>.md` → `<id>.progress.json`). */
function sidecarPath(mdPath: string): string {
  return mdPath.replace(/\.md$/, ".progress.json")
}

type Phase =
  | "history"
  | "idle"
  | "planning"
  | "confirm"
  | "generating"
  | "ready"
  | "no_match"
  | "error"
  | "attempt"
  | "review"

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
  /**
   * quiz-attempt-progress (design D6): wiki page index + navigate
   * handler, threaded to `QuizAnswering`/`QuizReview` so explanation
   * `[[slug]]` citations resolve and navigate to the wiki page.
   */
  wikiPages?: Record<string, WikiPageMeta>
  onOpenWikiPage?: (slug: string) => void
  /**
   * fix-quiz-ux-wiring (design D2): monotonic counter bumped by
   * Workspace when the user re-selects the already-active Quiz tab.
   * On any change to a value > 0, the Quiz tab returns to its
   * quiz-history view. The initial 0 is inert (does not yank a
   * freshly-mounted tab away from a flow). Non-destructive — answering
   * progress is persisted, so reopening an attempt resumes.
   */
  quizHomeSignal?: number
}


/**
 * Live plan/generate agent activity, rendered through the SAME stream
 * rendering used by the run detail / quiz generation-log views
 * (design D8). Nothing is shown until the first event arrives.
 */
function QuizLiveStream({ events }: { events: VerbEvent[] }) {
  if (events.length === 0) return null
  const folded = foldTimeline(events)
  return (
    <div
      data-testid="quiz-live-stream"
      className="flex flex-col gap-0.5 rounded-md border border-border bg-bg-sunken p-3"
    >
      {folded.map((item, i) =>
        item.kind === "thought_block" ? (
          <ThoughtItem key={i} text={item.text} />
        ) : (
          <ActivityStreamItem key={i} event={item.event} />
        ),
      )}
    </div>
  )
}

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
  wikiPages,
  onOpenWikiPage,
  quizHomeSignal,
}: QuizTabProps) {
  // Summary pass/fail threshold comes from app.quiz.pass_threshold via
  // the settings store (design D1) — never a hardcoded constant.
  const passThreshold = useSettingsStore((s) => s.getPassThreshold())
  const t = useT()
  const [phase, setPhase] = useState<Phase>("history")
  const [topic, setTopic] = useState("")
  const [pages, setPages] = useState<string[]>([])
  const [reason, setReason] = useState("")
  const [errorMsg, setErrorMsg] = useState("")
  const [quizMd, setQuizMd] = useState("")
  const [attempts, setAttempts] = useState<QuizAttemptMeta[]>([])
  // Derived per-attempt history badge keyed by attempt md path (design
  // D4). Recomputed from each attempt's sidecar + parsed question count
  // whenever the history list refreshes — never stored.
  const [badges, setBadges] = useState<Record<string, string>>({})
  const [attemptMd, setAttemptMd] = useState("")
  // The opened attempt's metadata (carries `events_log` so the attempt
  // detail view can offer the modal view-generation-log — design D9).
  const [attemptMeta, setAttemptMeta] = useState<QuizAttemptMeta | null>(null)
  // quiz-attempt-progress (design D3): the attempt md path whose sidecar
  // answering persists to, and the loaded progress to resume from.
  const [attemptPath, setAttemptPath] = useState("")
  const [attemptProgress, setAttemptProgress] =
    useState<QuizProgress | null>(null)
  const [logOpen, setLogOpen] = useState(false)
  // Live agent activity for the plan/generate spawn (design D8): the
  // `quiz-stream` VerbEvents, rendered through the existing stream
  // rendering during the planning/generating phases.
  const [liveEvents, setLiveEvents] = useState<VerbEvent[]>([])
  const unlistenRef = useRef<UnlistenFn | null>(null)
  const streamUnlistenRef = useRef<UnlistenFn | null>(null)
  // quiz-double-spawn-guard: latch the `pendingPage` value the Page-flow
  // effect already fired generation for, so a repeated effect invocation
  // with the same value (React StrictMode double-invoke in dev) does NOT
  // spawn a second quiz. Reset to null when `pendingPage` clears so the
  // same page can be re-quizzed in a later trigger.
  const firedForPageRef = useRef<string | null>(null)
  // Monotonic counter bumped by the quiz-changed watcher event. Included
  // in the history-load effect's deps so an external attempt write (e.g.
  // terminal `codebus quiz`) re-runs the load and the new row appears
  // without the user switching tabs. Spec: `Quiz Tab Subscribes To
  // Watcher Events`.
  const [historyRefreshKey, setHistoryRefreshKey] = useState(0)

  // Subscribe to the per-vault quiz history watcher. The list reload
  // happens via the dep-bump trick rather than a direct call so we
  // reuse the existing `phase === "history"` effect's loader.
  useEffect(
    () =>
      useWatcherEvent("quiz-changed", () => {
        setHistoryRefreshKey((k) => k + 1)
      }),
    [],
  )

  // Re-fetch the currently-open attempt's md + sidecar when the watcher
  // reports a change to that exact attempt. Edits to a different
  // attempt are ignored so the open attempt's UI never churns.
  useEffect(
    () =>
      useWatcherEvent("quiz-attempt-changed", (payload) => {
        if (!attemptPath) return
        // The watcher emits `slug` + `id`; the attemptPath ends with
        // `<slug>/<id>.md`. Normalize separators because Rust on Windows
        // may emit backslashes.
        const norm = attemptPath.replace(/\\/g, "/")
        const expectedSuffix = `/${payload.slug}/${payload.id}.md`
        if (!norm.endsWith(expectedSuffix)) return
        void Promise.all([
          readQuizAttempt(vaultPath, attemptPath),
          readQuizProgress(vaultPath, sidecarPath(attemptPath)),
        ])
          .then(([md, prog]) => {
            setAttemptMd(md)
            setAttemptProgress(prog)
          })
          .catch(() => {
            /* read race — non-fatal; next interaction reloads */
          })
      }),
    [attemptPath, vaultPath],
  )

  // Persist a submission to the open attempt's sidecar (design D3).
  // No-op when no attempt path is known (e.g. a generate whose persist
  // failed) — answering still works in-memory; only history loses it.
  function persistProgress(p: QuizProgress) {
    if (!attemptPath) return
    void writeQuizProgress(vaultPath, sidecarPath(attemptPath), p).catch(() => {
      /* sidecar write is best-effort — never break answering */
    })
  }

  // design D5 "Redo this": reset THIS attempt's sidecar to not-started
  // and re-enter answering at Q1 with the SAME generated questions. It
  // never re-spawns an agent (distinct from `+ New quiz`).
  async function redoThisAttempt() {
    const reset: QuizProgress = {
      schema_version: 1,
      answers: [],
      status: "not_started",
      started_at: null,
      completed_at: null,
    }
    if (attemptPath) {
      try {
        await writeQuizProgress(vaultPath, sidecarPath(attemptPath), reset)
      } catch {
        /* best-effort — still let the user re-answer in-memory */
      }
    }
    setAttemptProgress(null)
    setPhase("ready")
  }

  async function openAttempt(a: QuizAttemptMeta) {
    try {
      const md = await readQuizAttempt(vaultPath, a.path)
      const progress = await readQuizProgress(
        vaultPath,
        sidecarPath(a.path),
      )
      // Route by derived status (design D4): a completed attempt opens
      // the read-only attempt view; not-started / in-progress resumes
      // answering at the first unanswered question (design D3).
      if (progress.status === "completed") {
        setQuizMd(md)
        setAttemptPath(a.path)
        setAttemptProgress(progress)
        setAttemptMeta(a)
        setLogOpen(false)
        setPhase("review")
      } else {
        setQuizMd(md)
        setAttemptPath(a.path)
        setAttemptProgress(progress)
        setAttemptMeta(a)
        setPhase("ready")
      }
    } catch (e) {
      setErrorMsg(String(e))
      setPhase("error")
    }
  }

  // design D2 — Workspace bumps `quizHomeSignal` when the user
  // re-selects the already-active Quiz tab. Any change to a value > 0
  // returns the tab to its quiz-history view. The initial 0 is inert
  // so a freshly-mounted tab is not yanked out of a flow. Returning is
  // non-destructive: answering progress is persisted.
  useEffect(() => {
    if (quizHomeSignal && quizHomeSignal > 0) {
      setPhase("history")
    }
  }, [quizHomeSignal])

  // task 5.5 — refresh the history list whenever the history view is
  // shown (mount default, back from an attempt / input / answering).
  useEffect(() => {
    if (phase !== "history") return
    let alive = true
    void listQuizAttempts(vaultPath)
      .then(async (a) => {
        if (!alive) return
        setAttempts(a)
        // Derive each row's badge: parse the attempt md for the question
        // count (N), read its sidecar for status/answers (design D4).
        // Per-row reads run in parallel (design Risks: small JSON, same
        // order as the existing per-row reads — batching is later YAGNI).
        const entries = await Promise.all(
          a.map(async (m) => {
            try {
              const [md, prog] = await Promise.all([
                readQuizAttempt(vaultPath, m.path),
                readQuizProgress(vaultPath, sidecarPath(m.path)),
              ])
              const total = parseQuiz(md).length
              const correct = prog.answers.filter((x) => x.correct).length
              return [
                m.path,
                quizBadge(
                  prog.status,
                  prog.answers.length,
                  correct,
                  total,
                  passThreshold,
                  t,
                ),
              ] as const
            } catch {
              return [m.path, ""] as const
            }
          }),
        )
        if (alive) setBadges(Object.fromEntries(entries))
      })
      .catch(() => {
        /* missing dir / read error → leave list empty */
      })
    return () => {
      alive = false
    }
  }, [phase, vaultPath, passThreshold, historyRefreshKey, t])

  function clearListener() {
    if (unlistenRef.current) {
      unlistenRef.current()
      unlistenRef.current = null
    }
    if (streamUnlistenRef.current) {
      streamUnlistenRef.current()
      streamUnlistenRef.current = null
    }
  }

  // Subscribe to the live `quiz-stream` VerbEvents for the active
  // plan/generate spawn (design D8). Resets the accumulator so each
  // new run starts clean.
  async function subscribeQuizStream() {
    setLiveEvents([])
    streamUnlistenRef.current = await listen<QuizStreamPayload>(
      "quiz-stream",
      (event) => {
        setLiveEvents((prev) => [...prev, event.payload.event])
      },
    )
  }

  // task 5.3 Page flow: a content page's [Quiz me on this] sets
  // `pendingPage`. Skip planning entirely and generate directly with
  // pages=[pendingPage]. Fires on mount / prop change regardless of the
  // history default (design D5); consumed immediately (one-shot via
  // onPendingConsumed) so the same page can be re-quizzed later.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (!pendingPage) {
      // Cleared (consumed) — reset the latch so the same page can be
      // re-quizzed when `pendingPage` is set again later.
      firedForPageRef.current = null
      return
    }
    // StrictMode double-invoke / repeated render with the same value:
    // fire generation only once per distinct pendingPage value.
    if (firedForPageRef.current === pendingPage) return
    firedForPageRef.current = pendingPage
    onPendingConsumed?.()
    void startGenerate([pendingPage], {
      kind: "wiki_preview",
      target_page: pendingPage,
    })
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
    await subscribeQuizStream()
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
          // A fresh attempt starts not-started; its sidecar lives beside
          // the persisted md (design D3). `quiz_file` is null if persist
          // failed — then answering still works, just unpersisted.
          setAttemptPath(r.quiz_file ?? "")
          setAttemptProgress(null)
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
    await subscribeQuizStream()
    try {
      // Question count comes from the shared `quiz.default_length`
      // config (legacy `app.quiz.default_length` fallback, clamped
      // 3..10) — never a hardcoded constant (design D4). Read at spawn
      // time so it reflects the config loaded at workspace startup.
      await spawnQuizGenerate(
        vaultPath,
        genPages,
        useSettingsStore.getState().getDefaultLength(),
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
    setAttemptPath("")
    setAttemptProgress(null)
  }

  return (
    <div
      data-testid="quiz-tab"
      className="flex h-full w-full flex-col"
    >
      <WatcherStatusBanner vaultPath={vaultPath} />
      {/* Header mirrors GoalsTab exactly (full-width, border-b, p-3,
          pr-[160px] for the fixed WindowControls) so + New quiz lands
          in the same screen position/style as + New goal across tabs. */}
      <div
        data-tauri-drag-region
        className="flex items-center justify-between border-b border-border p-3 pr-[160px]"
      >
        <h2 className="text-body-lg font-medium text-fg-primary">
          {t("workspace.quiz.tab.heading")}
        </h2>
        {(phase === "history" || phase === "idle") && (
          <Button
            variant="primary"
            data-testid="new-quiz"
            onClick={() => setPhase("idle")}
          >
            {t("workspace.quiz.tab.newButton")}
          </Button>
        )}
      </div>

      <div className="flex flex-1 flex-col overflow-auto px-8 py-6">

      {phase === "history" && (
        <div
          data-testid="quiz-history"
          className="flex flex-1 flex-col overflow-auto"
        >
          {attempts.length === 0 ? (
            <p className="text-body-lg text-fg-secondary">
              {t("workspace.quiz.tab.emptyHint")}
            </p>
          ) : (
            groupBySlug(attempts).map(([slug, rows]) => (
              <div
                key={slug}
                data-testid="quiz-history-group"
                className="mb-3"
              >
                {/* QL1: group title shows the user-authored topic (Goal
                    flow) or target_page (Page flow); falls back to the
                    hash-derived slug only when neither is present so a
                    legacy attempt without frontmatter remains
                    identifiable. Spec: quiz § Quiz History Row Title
                    Displays User-Authored Topic. */}
                <p
                  data-testid="quiz-history-group-title"
                  className="text-body font-medium text-fg-primary"
                >
                  {rows[0]?.topic ?? rows[0]?.target_page ?? slug}
                </p>
                {rows.map((a) => (
                  <div
                    key={a.path}
                    data-testid="quiz-attempt-row"
                    className="flex items-center gap-2 py-1 text-body"
                  >
                    <button
                      type="button"
                      data-testid="quiz-attempt-open"
                      className="flex-1 text-left text-fg-secondary hover:text-fg-primary"
                      onClick={() => void openAttempt(a)}
                    >
                      {a.topic ?? a.target_page ?? a.slug} · {a.quiz_id}
                    </button>
                    <span
                      data-testid="quiz-attempt-badge"
                      className="shrink-0 tabular-nums text-fg-secondary"
                    >
                      {badges[a.path] ?? ""}
                    </span>
                  </div>
                ))}
              </div>
            ))
          )}
        </div>
      )}

      {phase === "idle" && (
        <div data-testid="quiz-idle" className="flex flex-col gap-3">
          <div>
            <Button
              data-testid="quiz-back-to-history"
              onClick={() => setPhase("history")}
            >
              {t("workspace.quiz.tab.backToHistoryShort")}
            </Button>
          </div>
          <input
            data-testid="quiz-topic-input"
            className="rounded border border-border bg-bg-secondary px-3 py-2 text-body-lg"
            placeholder={t("workspace.quiz.tab.topicPlaceholder")}
            value={topic}
            onChange={(e) => setTopic(e.target.value)}
          />
          <div>
            <Button
              data-testid="quiz-start"
              onClick={onStart}
              disabled={!topic.trim()}
            >
              {t("workspace.quiz.tab.startButton")}
            </Button>
          </div>
        </div>
      )}

      {phase === "attempt" && (
        <div
          data-testid="quiz-attempt-view"
          className="flex flex-1 flex-col gap-2"
        >
          <div className="flex gap-2">
            <Button
              data-testid="quiz-attempt-back"
              onClick={() => {
                setAttemptMd("")
                setPhase("history")
              }}
            >
              {t("workspace.quiz.tab.backToHistoryFull")}
            </Button>
            {attemptMeta?.events_log && (
              <Button
                variant="secondary"
                data-testid="quiz-view-log"
                onClick={() => setLogOpen(true)}
              >
                {t("workspace.quiz.review.viewLogButton")}
              </Button>
            )}
          </div>
          <pre className="flex-1 overflow-auto rounded bg-bg-secondary p-3 text-meta">
            {attemptMd}
          </pre>
          {attemptMeta?.events_log && (
            <Dialog
              open={logOpen}
              onOpenChange={(o) => setLogOpen(o)}
            >
              <DialogContent data-testid="quiz-view-log-modal">
                <DialogHeader>
                  <DialogTitle>
                    {t("workspace.quiz.review.generationLogTitle")}
                  </DialogTitle>
                </DialogHeader>
                <div className="max-h-[60vh] overflow-auto">
                  <QuizGenerationLog
                    vaultPath={vaultPath}
                    eventsLog={attemptMeta.events_log}
                  />
                </div>
                <DialogClose asChild>
                  <Button
                    variant="secondary"
                    data-testid="quiz-view-log-close"
                  >
                    {t("workspace.quiz.review.viewLogClose")}
                  </Button>
                </DialogClose>
              </DialogContent>
            </Dialog>
          )}
        </div>
      )}

      {phase === "review" && (
        <QuizReview
          quizMd={quizMd}
          progress={attemptProgress ?? { schema_version: 1, answers: [], status: "completed", started_at: null, completed_at: null }}
          passThreshold={passThreshold}
          vaultPath={vaultPath}
          eventsLog={attemptMeta?.events_log ?? null}
          pages={wikiPages}
          onOpenWikiPage={onOpenWikiPage}
          onRedo={() => void redoThisAttempt()}
          onBack={() => {
            setQuizMd("")
            setAttemptProgress(null)
            setPhase("history")
          }}
        />
      )}

      {phase === "planning" && (
        <div data-testid="quiz-planning" className="flex flex-col gap-2">
          <p className="text-body-lg text-fg-secondary">
            {t("workspace.quiz.tab.planningStatus")}
          </p>
          <QuizLiveStream events={liveEvents} />
        </div>
      )}

      {phase === "confirm" && (
        <div data-testid="quiz-confirm" className="flex flex-col gap-3">
          <p
            data-testid="quiz-confirm-desc"
            className="text-body-lg text-fg-primary"
          >
            {t("workspace.quiz.confirmDescription")}
          </p>
          <ul className="text-body text-fg-secondary">
            {pages.map((p) => (
              <li key={p} data-testid="quiz-scope-page">
                {p}
              </li>
            ))}
          </ul>
          <div className="flex gap-2">
            <Button data-testid="quiz-revise" onClick={reset}>
              {t("workspace.quiz.revise")}
            </Button>
            <Button data-testid="quiz-generate" onClick={onConfirm}>
              {t("workspace.quiz.confirm")}
            </Button>
          </div>
        </div>
      )}

      {phase === "generating" && (
        <div data-testid="quiz-generating" className="flex flex-col gap-2">
          <p className="text-body-lg text-fg-secondary">
            {t("workspace.quiz.tab.generatingStatus")}
          </p>
          <QuizLiveStream events={liveEvents} />
        </div>
      )}

      {phase === "ready" && (
        <div data-testid="quiz-ready" className="flex flex-1 flex-col">
          {/*
           * Back-to-history control (design D1). Wraps the answering
           * view so it is reachable during answering AND on the
           * post-quiz summary. Same testid + `setPhase("history")`
           * behavior as the idle-phase control; the two phases are
           * mutually exclusive so they never render simultaneously.
           * Non-destructive: answering progress is persisted by the
           * cursor sidecar, so reopening the attempt resumes exactly.
           * It does NOT spawn an agent.
           */}
          <div className="mb-2">
            <Button
              data-testid="quiz-back-to-history"
              onClick={() => setPhase("history")}
            >
              {t("workspace.quiz.tab.backToHistoryShort")}
            </Button>
          </div>
          <QuizAnswering
            quizMd={quizMd}
            passThreshold={passThreshold}
            pages={wikiPages}
            onOpenWikiPage={onOpenWikiPage}
            initialProgress={attemptProgress}
            onPersist={persistProgress}
          />
        </div>
      )}

      {phase === "no_match" && (
        <div data-testid="quiz-no-match" className="flex flex-col gap-3">
          <p className="text-body-lg text-fg-primary">
            {t("workspace.quiz.tab.noMatchPrefix", { reason })}
          </p>
          <div>
            <Button data-testid="quiz-back" onClick={reset}>
              {t("workspace.quiz.tab.backButton")}
            </Button>
          </div>
        </div>
      )}

      {phase === "error" && (
        <div data-testid="quiz-error" className="flex flex-col gap-3">
          <p className="text-body-lg text-fg-primary">
            {t("workspace.quiz.tab.errorPrefix", { message: errorMsg })}
          </p>
          <div>
            <Button data-testid="quiz-back" onClick={reset}>
              {t("workspace.quiz.tab.backButton")}
            </Button>
          </div>
        </div>
      )}
      </div>
    </div>
  )
}
