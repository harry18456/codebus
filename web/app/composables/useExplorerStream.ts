import { ref, watch, type Ref } from 'vue'
import { useSseTask, type SseEvent, type SseStatus } from './useSseTask'
import type { AuditRow } from '~/components/audit/AuditPanel.vue'

// useExplorerStream — single SSE dispatch entry for the Module 4 Explorer
// console (change agent-console-p0). Wraps exactly one useSseTask instance
// and fans its events into bucket-fill reactive state. Pages MUST construct
// at most one instance per `task_id`; never instantiate alongside a separate
// EventSource for the same task.

export interface ToolCall {
  tool: string
  args: Record<string, unknown>
}

export interface ActionEntry {
  tool: string
  observation: string
  tokens_used: number
  isError: boolean
}

export interface StepBucket {
  step: number
  thought?: { text: string; actions: ToolCall[] }
  actions: ActionEntry[]
  judge?: { relevance: number; reason: string }
}

export interface ProgressSnapshot {
  current: number
  total: number
}

export interface CoverageGap {
  description: string
  suggested_target: string | null
}

export type CoverageSkipReason =
  | 'no_gaps'
  | 'budget_exhausted'
  | 'max_depth_reached'
  | null

export interface CoverageBannerEvent {
  round: number
  gaps: CoverageGap[]
  will_recurse: boolean
  skip_reason: CoverageSkipReason
}

export type BudgetWarningKind = 'tokens' | 'steps'

export interface BudgetWarningEvent {
  kind: BudgetWarningKind
  current: number
  budget: number
  pct: number
}

export interface BudgetBannerState {
  tokens?: BudgetWarningEvent
  steps?: BudgetWarningEvent
}

export interface UseExplorerStreamApi {
  stepBuckets: Ref<Map<number, StepBucket>>
  progress: Ref<ProgressSnapshot | null>
  coverageBanner: Ref<CoverageBannerEvent | null>
  budgetBanner: Ref<BudgetBannerState>
  auditRows: Ref<AuditRow[]>
  status: Ref<SseStatus>
  error: Ref<Error | null>
  done: Ref<boolean>
  close: () => void
}

const AUDIT_WINDOW_CAP = 200
const ERROR_PREFIX = 'error:'
const TRACEBACK_MARKER = 'traceback'

function isObservationError(observation: string): boolean {
  return (
    observation.startsWith(ERROR_PREFIX) ||
    observation.toLowerCase().includes(TRACEBACK_MARKER)
  )
}

function ensureBucket(map: Map<number, StepBucket>, step: number): StepBucket {
  let b = map.get(step)
  if (!b) {
    b = { step, actions: [] }
    map.set(step, b)
  }
  return b
}

function nowTs(): string {
  // HH:MM:SS in local time, matching AuditPanel mockup convention.
  const d = new Date()
  const pad = (n: number): string => String(n).padStart(2, '0')
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

interface ThoughtPayload {
  step: number
  thought: string
  action: ToolCall[]
}
interface ActionResultPayload {
  step: number
  tool: string
  observation: string
  tokens_used: number
}
interface JudgePayload {
  step: number
  relevance: number
  reason: string
}

function isThought(d: unknown): d is ThoughtPayload {
  return (
    typeof d === 'object' &&
    d !== null &&
    typeof (d as ThoughtPayload).step === 'number' &&
    typeof (d as ThoughtPayload).thought === 'string'
  )
}
function isActionResult(d: unknown): d is ActionResultPayload {
  return (
    typeof d === 'object' &&
    d !== null &&
    typeof (d as ActionResultPayload).step === 'number' &&
    typeof (d as ActionResultPayload).tool === 'string' &&
    typeof (d as ActionResultPayload).observation === 'string'
  )
}
function isJudge(d: unknown): d is JudgePayload {
  return (
    typeof d === 'object' &&
    d !== null &&
    typeof (d as JudgePayload).step === 'number' &&
    typeof (d as JudgePayload).relevance === 'number'
  )
}
function isProgress(
  d: unknown
): d is { phase: string; current: number; total: number } {
  return (
    typeof d === 'object' &&
    d !== null &&
    typeof (d as { phase: unknown }).phase === 'string' &&
    typeof (d as { current: unknown }).current === 'number' &&
    typeof (d as { total: unknown }).total === 'number'
  )
}
function isCoverage(d: unknown): d is CoverageBannerEvent {
  return (
    typeof d === 'object' &&
    d !== null &&
    Array.isArray((d as CoverageBannerEvent).gaps) &&
    typeof (d as CoverageBannerEvent).round === 'number'
  )
}
function isBudget(d: unknown): d is BudgetWarningEvent {
  const kind = (d as BudgetWarningEvent | null)?.kind
  return kind === 'tokens' || kind === 'steps'
}

export function useExplorerStream(taskId: string): UseExplorerStreamApi {
  const sse = useSseTask(taskId)

  const stepBuckets: Ref<Map<number, StepBucket>> = ref(new Map())
  const progress: Ref<ProgressSnapshot | null> = ref(null)
  const coverageBanner: Ref<CoverageBannerEvent | null> = ref(null)
  const budgetBanner: Ref<BudgetBannerState> = ref({})
  const auditRows: Ref<AuditRow[]> = ref([])
  const done: Ref<boolean> = ref(false)

  let cursor = 0

  function pushAudit(body: string): void {
    auditRows.value.push({ ts: nowTs(), body })
    while (auditRows.value.length > AUDIT_WINDOW_CAP) {
      auditRows.value.shift()
    }
  }

  function dispatch(ev: SseEvent): void {
    switch (ev.type) {
      case 'agent_thought':
        if (isThought(ev.data)) {
          const b = ensureBucket(stepBuckets.value, ev.data.step)
          b.thought = { text: ev.data.thought, actions: ev.data.action ?? [] }
          // Trigger reactive update for Map mutation (Vue tracks .set
          // additions but not in-place property writes on existing values).
          stepBuckets.value = new Map(stepBuckets.value)
          pushAudit(`step ${ev.data.step} · think · ${ev.data.thought}`)
        }
        break
      case 'agent_action_result':
        if (isActionResult(ev.data)) {
          const b = ensureBucket(stepBuckets.value, ev.data.step)
          b.actions.push({
            tool: ev.data.tool,
            observation: ev.data.observation,
            tokens_used: ev.data.tokens_used,
            isError: isObservationError(ev.data.observation)
          })
          stepBuckets.value = new Map(stepBuckets.value)
          pushAudit(
            `step ${ev.data.step} · act · ${ev.data.tool} → ${ev.data.observation}`
          )
        }
        break
      case 'judge_verdict':
        if (isJudge(ev.data)) {
          const b = ensureBucket(stepBuckets.value, ev.data.step)
          b.judge = { relevance: ev.data.relevance, reason: ev.data.reason }
          stepBuckets.value = new Map(stepBuckets.value)
          pushAudit(
            `step ${ev.data.step} · judge · ${ev.data.relevance.toFixed(2)} — ${ev.data.reason}`
          )
        }
        break
      case 'progress':
        if (isProgress(ev.data) && ev.data.phase === 'exploring') {
          progress.value = { current: ev.data.current, total: ev.data.total }
        }
        break
      case 'coverage_gaps':
        if (isCoverage(ev.data)) {
          coverageBanner.value = ev.data
        }
        break
      case 'budget_warning':
        if (isBudget(ev.data)) {
          budgetBanner.value = {
            ...budgetBanner.value,
            [ev.data.kind]: ev.data
          }
        }
        break
      case 'done':
        if (!done.value) {
          done.value = true
        }
        break
      default:
        // usage_delta / llm_call / qa_answer / kb_growth / rag_hits / message
        // are out of scope for the explorer console P0; ignore silently.
        break
    }
  }

  const stopWatch = watch(
    () => sse.events.value.length,
    (len) => {
      while (cursor < len) {
        const ev = sse.events.value[cursor]
        if (ev) dispatch(ev)
        cursor += 1
      }
    },
    { immediate: true }
  )

  function close(): void {
    stopWatch()
    sse.close()
  }

  return {
    stepBuckets,
    progress,
    coverageBanner,
    budgetBanner,
    auditRows,
    status: sse.status,
    error: sse.error,
    done,
    close
  }
}
