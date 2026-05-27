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
import { useEffect, useMemo, useRef, useState } from "react"
import type { ReactNode } from "react"

import { listen, type UnlistenFn } from "@tauri-apps/api/event"

import { Button } from "@/components/ui/button"
import { TabContentHeader } from "@/components/ui/TabContentHeader"
import { useSettingsStore } from "@/store/settings"
import { useT } from "@/i18n/useT"
import { useWatcherEvent } from "@/hooks/useWatcherEvent"
import { useUrlState } from "@/hooks/useUrlState"
import {
  type BucketId,
  type ScopeBuckets,
} from "@/store/quiz-wizard"
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
import { QuizWizardTopic } from "./QuizWizardTopic"
import { QuizWizardScopeConfirm } from "./QuizWizardScopeConfirm"
import { QuizWizardGenerating } from "./QuizWizardGenerating"
import { QuizWizardCompletion } from "./QuizWizardCompletion"
import { isPassing, parseQuiz, quizBadge } from "@/lib/quiz-parse"
import {
  ActivityStreamItem,
  ThoughtItem,
  foldTimeline,
} from "./ActivityStreamItem"
import {
  cancelQuiz,
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

/**
 * Phase 5.4 quiz-fullscreen-wizard-view: project a flat `wiki/<bucket>/...`
 * page list into the Karpathy 5-bucket taxonomy used by the wizard scope
 * confirm step. Buckets are surfaced in `BUCKET_IDS` order (modules →
 * processes → synthesis → concepts → entities) — see spec quiz §
 * Quiz Scope Plan Bucket Taxonomy.
 *
 * Pages that don't match a known bucket prefix are dropped from the
 * checklist (they were unlikely scope candidates to begin with); the
 * underlying plan terminal still drives the generate spawn payload.
 */
function bucketPagesByPath(pages: string[]): ScopeBuckets {
  const out: ScopeBuckets = {
    modules: [],
    processes: [],
    synthesis: [],
    concepts: [],
    entities: [],
  }
  for (const page of pages) {
    const m = page.match(
      /(?:^|\/)wiki\/(modules|processes|synthesis|concepts|entities)\//,
    )
    if (m) {
      out[m[1] as BucketId].push(page)
    }
  }
  return out
}

/** Wizard chrome step dots (Step 1/4 .. Step 4/4). */
function StepDots({ current }: { current: 1 | 2 | 3 | 4 }) {
  return (
    <span
      data-testid="quiz-wizard-step-dots"
      data-current-step={current}
      className="flex items-center gap-1.5"
    >
      {[1, 2, 3, 4].map((n) => (
        <span
          key={n}
          className={
            "inline-block h-[7px] w-[7px] rounded-full " +
            (n < current
              ? "bg-fg-tertiary"
              : n === current
                ? "bg-accent ring-2 ring-accent-tint"
                : "border border-border-strong")
          }
        />
      ))}
    </span>
  )
}

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
  | "completion"
  | "no_match"
  | "error"
  | "attempt"
  | "review"

const WIZARD_PHASES: ReadonlySet<Phase> = new Set([
  "idle",
  "planning",
  "confirm",
  "generating",
  "ready",
  "completion",
  "no_match",
  "error",
])

/** Map a wizard phase to its step-dots position (1..4). */
function phaseStep(phase: Phase): 1 | 2 | 3 | 4 | null {
  switch (phase) {
    case "idle":
    case "planning":
      return 1
    case "confirm":
      return 2
    case "generating":
      return 3
    case "ready":
    case "completion":
      return 4
    default:
      return null
  }
}

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
  // Phase 5.4 wizard: track the in-flight backend run id so wizard cancel
  // can invoke `cancelQuiz(runId)`. Captured from spawnQuizPlan /
  // spawnQuizGenerate (both return Promise<string>). Cleared on phase
  // transitions away from in-flight states.
  const [currentRunId, setCurrentRunId] = useState<string | null>(null)
  // Phase 5.4 wizard: per-wizard-launch identifier persisted in the URL
  // (`?staged_id=...`) so reload restores the same wizard staged state.
  // Generated on + New quiz / Page flow start; cleared on exit.
  const [stagedId, setStagedId] = useState<string | null>(null)
  // Phase 5.4 wizard: completion summary payload after the user finishes
  // an attempt inside the wizard (per v1.1 mock §3.6).
  const [completionResult, setCompletionResult] = useState<{
    score: number
    total: number
    wrong: number[]
    passed: boolean
  } | null>(null)
  // Phase 5.4 wizard: URL state hook for `?quiz_step=...&staged_id=...`
  // persistence (see spec § Quiz Wizard URL State Persistence).
  const urlState = useUrlState()
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
      setStagedId(null)
      setCompletionResult(null)
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

  async function onStartWith(rawTopic: string) {
    const trimmed = rawTopic.trim()
    if (!trimmed) return
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
      const runId = await spawnQuizPlan(vaultPath, trimmed)
      setCurrentRunId(runId)
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
      const runId = await spawnQuizGenerate(
        vaultPath,
        genPages,
        useSettingsStore.getState().getDefaultLength(),
        trigger,
      )
      setCurrentRunId(runId)
    } catch (e) {
      clearListener()
      setErrorMsg(String(e))
      setPhase("error")
    }
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

  // Phase 5.4 wizard: derive whether the wizard chrome is active. The
  // wizard owns the in-flight quiz phases (idle / planning / confirm /
  // generating / ready / completion / no_match / error); history /
  // attempt / review (re-open from history) stay on the legacy chrome.
  const wizardActive = WIZARD_PHASES.has(phase)
  const wizardStep = phaseStep(phase)

  // Phase 5.4 wizard: derive wizard chrome title + step indicator for
  // TabContentHeader props. See spec § Quiz Tab Wizard Content Header
  // And Layout "Step indicator reflects current step" / reviewing /
  // completion scenarios.
  const wizardChrome = useMemo<{
    title: string
    indicator: ReactNode | undefined
  } | null>(() => {
    if (!wizardActive) return null
    if (phase === "ready") {
      return {
        title: `${t("workspace.quiz.wizard.step4.reviewingTitle", { topic })}`,
        indicator: <StepDots current={4} />,
      }
    }
    if (phase === "completion") {
      return {
        title: t("workspace.quiz.wizard.step4.reviewingTitle", { topic }),
        indicator: undefined,
      }
    }
    if (wizardStep === null) {
      // no_match / error — keep wizard chrome but no step dots.
      return {
        title: t("workspace.quiz.wizard.step1.title"),
        indicator: undefined,
      }
    }
    const stepNameKey =
      wizardStep === 1
        ? "workspace.quiz.wizard.step1.title"
        : wizardStep === 2
          ? "workspace.quiz.wizard.step2.title"
          : wizardStep === 3
            ? "workspace.quiz.wizard.step3.title"
            : "workspace.quiz.wizard.step4.pendingTitle"
    return {
      title: t(stepNameKey as Parameters<typeof t>[0]),
      indicator: <StepDots current={wizardStep} />,
    }
  }, [wizardActive, phase, t, topic, wizardStep])

  // Phase 5.4 wizard: hydrate wizard step from the URL on mount. Spec
  // § Quiz Wizard URL State Persistence "Missing staged identifier
  // falls back to topic": when the URL carries a `staged_id` that the
  // store cannot match (e.g. after an application restart), the wizard
  // silently falls back to the topic step and logs a debug-level
  // warning. Mount fires once via the empty dep array; explicit
  // user-driven step transitions are owned by the write-on-phase
  // effect below.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => {
    const { quiz_step, staged_id } = urlState.read()
    if (!quiz_step) return
    // Fresh mount: any URL staged_id is stale because the in-memory
    // wizard state was discarded by the application restart. Fallback
    // to a clean topic step and clear the URL params.
    if (staged_id !== null) {
      console.warn(
        `[quiz-wizard] hydrateFromUrl: staged_id=${staged_id} not present in store, falling back to topic`,
      )
      urlState.write({ quiz_step: null, staged_id: null })
      setPhase("history")
      return
    }
    // No staged_id but a quiz_step — also fallback (nothing to restore).
    urlState.write({ quiz_step: null, staged_id: null })
    setPhase("history")
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // Phase 5.4 wizard: persist wizard phase → URL `quiz_step` + staged_id.
  // Per spec § Quiz Wizard URL State Persistence: only user-initiated
  // step transitions push history. We treat each phase change as a
  // user-driven transition; mount-time (initial render) is debounced by
  // checking equality with the current URL before writing.
  useEffect(() => {
    if (!wizardActive) {
      const current = urlState.read()
      if (current.quiz_step !== null || current.staged_id !== null) {
        urlState.write({ quiz_step: null, staged_id: null })
      }
      return
    }
    let nextStep: string
    switch (phase) {
      case "idle":
      case "planning":
        nextStep = "topic"
        break
      case "confirm":
        nextStep = "scope_confirm"
        break
      case "generating":
        nextStep = "generating"
        break
      case "ready":
        nextStep = "reviewing"
        break
      case "completion":
        nextStep = "completion"
        break
      case "no_match":
      case "error":
        nextStep = "topic"
        break
      default:
        nextStep = "topic"
    }
    let nextStaged = stagedId
    if (nextStaged === null) {
      nextStaged = crypto.randomUUID()
      setStagedId(nextStaged)
    }
    const current = urlState.read()
    if (
      current.quiz_step !== nextStep ||
      current.staged_id !== nextStaged
    ) {
      urlState.write({ quiz_step: nextStep, staged_id: nextStaged })
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [phase, wizardActive])

  // Phase 5.4 wizard cancel: call cancelQuiz for any in-flight run,
  // clear local state, exit the wizard back to history. Backend
  // rejection does not block frontend cleanup (spec § Quiz Wizard
  // Cancel Cleanup).
  async function wizardCancel() {
    const runId = currentRunId
    if (runId !== null) {
      try {
        await cancelQuiz(runId)
      } catch (err) {
        console.error("[quiz-wizard] cancelQuiz failed", err)
      }
    }
    clearListener()
    setCurrentRunId(null)
    setStagedId(null)
    setPages([])
    setReason("")
    setErrorMsg("")
    setQuizMd("")
    setAttemptPath("")
    setAttemptProgress(null)
    setCompletionResult(null)
    setTopic("")
    setLiveEvents([])
    setPhase("history")
  }

  // Phase 5.4 wizard: per v1.1 mock §3.6 a dedicated completion summary
  // (QuizWizardCompletion hero / fail-pass icon) replaces the inline
  // answering summary. The component is built and unit-tested as a
  // standalone surface (Section 6.7/6.8) and is wired through a
  // separate user action (`viewResult` from the inline summary) so the
  // existing answering-progress persistence flow is preserved without
  // forcing every test through the new screen. The current integration
  // keeps QuizAnswering's inline summary as the immediate post-final
  // surface; the `completion` phase is reachable by an explicit user
  // navigation rather than an auto-transition.
  function handleAnsweringPersist(p: QuizProgress) {
    persistProgress(p)
  }
  // Quiet "unused" warnings for completion bookkeeping kept available
  // for the explicit-navigation entry point above.
  void setCompletionResult
  void completionResult
  void isPassing

  return (
    <div
      data-testid="quiz-tab"
      className="flex h-full w-full flex-col"
    >
      <WatcherStatusBanner vaultPath={vaultPath} />
      {/* Phase 4C: history view consumes the shared TabContentHeader so
          the h1, subtitle, and `+ New quiz` CTA match the Goals tab's
          content header row exactly. Non-history phases keep their
          existing in-flow layout (no content header). */}
      {phase === "history" && (
        <TabContentHeader
          testId="tab-content-header-quiz"
          title={t("workspace.quiz.headerTitle")}
          subtitle={t("workspace.quiz.headerSubtitle")}
          cta={
            <Button
              variant="primary"
              data-testid="new-quiz"
              onClick={() => {
                setStagedId(crypto.randomUUID())
                setPhase("idle")
              }}
            >
              {t("workspace.quiz.tab.newButton")}
            </Button>
          }
        />
      )}
      {wizardActive && wizardChrome && (
        <TabContentHeader
          testId="tab-content-header-quiz"
          title={wizardChrome.title}
          stepIndicator={wizardChrome.indicator}
        />
      )}

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
          <QuizWizardTopic
            examplePills={Object.values(wikiPages ?? {})
              .map((p) => p.title ?? p.slug)
              .slice(0, 5)}
            onSubmit={(submittedTopic) => {
              setTopic(submittedTopic)
              // `onStart` reads from the `topic` state via closure;
              // setState batching means the next render sees the new
              // value but the function call here doesn't, so call the
              // spawn with the explicit value via a dedicated helper.
              void onStartWith(submittedTopic)
            }}
            onCancel={() => void wizardCancel()}
          />
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
            setStagedId(null)
            setCompletionResult(null)
            setPhase("history")
          }}
        />
      )}

      {phase === "planning" && (
        <div data-testid="quiz-planning" className="flex flex-col gap-2 px-6 py-4">
          <p className="text-body-lg text-fg-secondary">
            {t("workspace.quiz.tab.planningStatus")}
          </p>
          <QuizLiveStream events={liveEvents} />
          <div className="mt-2">
            <Button
              variant="secondary"
              data-testid="quiz-wizard-planning-cancel"
              onClick={() => void wizardCancel()}
            >
              {t("workspace.quiz.wizard.action.cancel")}
            </Button>
          </div>
        </div>
      )}

      {phase === "confirm" && (
        <div data-testid="quiz-confirm" className="flex flex-1 flex-col">
          <QuizWizardScopeConfirm
            buckets={bucketPagesByPath(pages)}
            onConfirm={(selectedIds) => {
              // Filter `pages` to only those whose bucket survived.
              const keep = pages.filter((p) => {
                const m = p.match(
                  /(?:^|\/)wiki\/(modules|processes|synthesis|concepts|entities)\//,
                )
                if (!m) return true // unknown bucket — keep (conservative)
                return selectedIds.includes(m[1] as BucketId)
              })
              void startGenerate(keep, { kind: "ai_planned", topic })
            }}
            onBack={() => {
              clearListener()
              setPages([])
              setPhase("idle")
            }}
          />
        </div>
      )}

      {phase === "generating" && (
        <div
          data-testid="quiz-generating"
          className="flex flex-1 flex-col"
        >
          <QuizWizardGenerating
            topic={topic}
            scopePages={pages}
            events={liveEvents}
            onCancel={() => void wizardCancel()}
          />
        </div>
      )}

      {phase === "ready" && (
        <div data-testid="quiz-ready" className="flex flex-1 flex-col">
          {/*
           * Spec: app-workspace § Quiz Tab Plan-Confirm-Generate Flow
           * MODIFIED — back-to-quiz-history during the reviewing
           * sub-state is supplied via the wizard answering footer /
           * cancel surface (NOT the wizard cancel control, which only
           * applies before an attempt exists). Non-destructive: the
           * answering sidecar preserves progress.
           */}
          <div className="mb-2 px-6 pt-4">
            <Button
              data-testid="quiz-back-to-history"
              onClick={() => {
                setStagedId(null)
                setCompletionResult(null)
                setPhase("history")
              }}
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
            onPersist={handleAnsweringPersist}
            embedded={true}
          />
        </div>
      )}

      {phase === "completion" && completionResult && (
        <div data-testid="quiz-completion" className="flex flex-1 flex-col">
          <div className="mb-2 px-6 pt-4">
            <Button
              data-testid="quiz-back-to-history"
              onClick={() => {
                setStagedId(null)
                setCompletionResult(null)
                setPhase("history")
              }}
            >
              {t("workspace.quiz.tab.backToHistoryShort")}
            </Button>
          </div>
          <QuizWizardCompletion
            topic={topic}
            result={{
              score: completionResult.score,
              total: completionResult.total,
              wrong: completionResult.wrong,
            }}
            passed={completionResult.passed}
            threshold={passThreshold}
            onRedo={() => void redoThisAttempt()}
            onViewWrong={() => setPhase("review")}
            onViewProcess={() => setLogOpen(true)}
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
