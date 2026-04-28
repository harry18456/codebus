// route.json loader + station_id → file_path resolver. Pages call
// loadRoute() once on mount, then findStationFile() to resolve each
// station_id to a relative markdown path. TypeScript types align with
// D-029 schema (`module-5-generator` capability §八).

import { useTutorialFiles } from './useTutorialFiles'

export interface RouteStation {
  station_id: string
  index: number
  title: string
  duration: number
  file_path: string
  required_checks: string[]
  related_stations: string[]
  degraded: boolean
  prerequisites?: string[]
}

export interface RouteJson {
  stations: RouteStation[]
}

export interface TaskResolution {
  task_id: string | null
  source: 'query' | 'single' | 'latest' | 'empty'
}

const TASK_ID_RE = /^generate_[0-9a-f]{8}$/

async function loadRoute(workspaceRoot: string, taskId: string): Promise<RouteJson> {
  const files = useTutorialFiles()
  const raw = await files.readTutorialFile(
    workspaceRoot,
    `codebus-tutorials/${taskId}/route.json`
  )
  const parsed = JSON.parse(raw) as RouteJson
  if (!Array.isArray(parsed.stations)) {
    throw new Error('route.json missing stations[]')
  }
  return parsed
}

function findStation(route: RouteJson, stationId: string): RouteStation | null {
  return route.stations.find((s) => s.station_id === stationId) ?? null
}

function findStationFile(route: RouteJson, stationId: string): string | null {
  const s = findStation(route, stationId)
  return s ? s.file_path : null
}

/**
 * Implicit-latest task resolution per design D-T11.
 *
 * 1. Honour `?task=<id>` when valid + directory exists
 * 2. Otherwise scan `<ws>/codebus-tutorials/*` and pick the entry whose
 *    `tutorial.md` frontmatter `generated_at` is most recent (fallback:
 *    directory mtime). Single-task workspaces auto-pick. Empty
 *    workspaces return `{ task_id: null, source: 'empty' }` so the page
 *    can render the empty CTA (D-T13) instead of an error view.
 */
async function resolveTaskId(
  workspaceRoot: string,
  queryTask: string | null
): Promise<TaskResolution> {
  const files = useTutorialFiles()

  if (queryTask && TASK_ID_RE.test(queryTask)) {
    try {
      // Probe by attempting to read route.json — if it succeeds the
      // directory exists and is well-formed. If not, fall through.
      await files.readTutorialFile(
        workspaceRoot,
        `codebus-tutorials/${queryTask}/route.json`
      )
      return { task_id: queryTask, source: 'query' }
    } catch {
      // Fall through to implicit-latest scan.
    }
  }

  const tasks = await files.listTutorialTasks(workspaceRoot)
  if (tasks.length === 0) {
    return { task_id: null, source: 'empty' }
  }
  if (tasks.length === 1) {
    return { task_id: tasks[0]!.id, source: 'single' }
  }

  // Sort by frontmatter generated_at desc; fall back to dir_mtime_unix
  // when frontmatter is absent or unparseable.
  const sorted = [...tasks].sort((a, b) => {
    const ta = parseGeneratedAt(a.frontmatter_raw) ?? a.dir_mtime_unix
    const tb = parseGeneratedAt(b.frontmatter_raw) ?? b.dir_mtime_unix
    return tb - ta
  })
  return { task_id: sorted[0]!.id, source: 'latest' }
}

/** Parse `generated_at: <ISO-8601>` line from a raw frontmatter block.
 *  Returns Unix seconds, or null if absent / unparseable. */
function parseGeneratedAt(rawFrontmatter: string | null): number | null {
  if (!rawFrontmatter) return null
  const match = rawFrontmatter.match(/^generated_at:\s*"?([^"\n]+?)"?\s*$/m)
  if (!match || !match[1]) return null
  const ts = Date.parse(match[1])
  return Number.isFinite(ts) ? Math.floor(ts / 1000) : null
}

interface StationRouteApi {
  loadRoute: typeof loadRoute
  findStation: typeof findStation
  findStationFile: typeof findStationFile
  resolveTaskId: typeof resolveTaskId
}

export function useStationRoute(): StationRouteApi {
  return { loadRoute, findStation, findStationFile, resolveTaskId }
}
