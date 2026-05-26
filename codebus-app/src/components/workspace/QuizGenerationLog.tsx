/**
 * Quiz generation-log view (fix-app-quiz task 2.4).
 *
 * Spec: app-workspace § Quiz History List —
 * "View-generation-log opens the events timeline" /
 * "View-generation-log is not a bare path".
 *
 * Loads the attempt's generate-spawn events.jsonl via the
 * `read_quiz_events` IPC and replays it through the SAME agent stream
 * rendering used by the run detail view (`foldTimeline` +
 * `ThoughtItem` / `ActivityStreamItem`) — never a bare path string.
 */
import { useEffect, useMemo, useState } from "react"

import { readQuizEvents, type EventEnvelope, type VerbEvent } from "@/lib/ipc"

import { ActivityStreamItem, ThoughtItem, foldTimeline } from "./ActivityStreamItem"

interface QuizGenerationLogProps {
  vaultPath: string
  eventsLog: string
}

export function QuizGenerationLog({
  vaultPath,
  eventsLog,
}: QuizGenerationLogProps) {
  const [events, setEvents] = useState<EventEnvelope[]>([])
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    readQuizEvents(vaultPath, eventsLog)
      .then((envs) => {
        if (!cancelled) setEvents(envs)
      })
      .catch((e) => {
        if (!cancelled) setError(String(e))
      })
    return () => {
      cancelled = true
    }
  }, [vaultPath, eventsLog])

  const timeline = useMemo<readonly VerbEvent[]>(
    () => events.map((env) => env.event),
    [events],
  )
  const folded = useMemo(() => foldTimeline(timeline), [timeline])

  if (error) {
    return (
      <div
        data-testid="quiz-generation-log-error"
        className="p-4 text-body text-red-500"
      >
        Could not load generation log: {error}
      </div>
    )
  }

  return (
    <div
      data-testid="quiz-generation-log"
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
