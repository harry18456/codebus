// Tauri IPC wrapper for tutorial file IO. Goes to the three Rust
// commands in `tauri/src-tauri/src/tutorial.rs`; design D-T1 / D-T11.
//
// All file reads / writes for `<ws>/codebus-tutorials/` MUST go through
// this composable so the Rust trust boundary (validate_path + symlink
// rejection + extension allowlist) is the single enforcement point.
// Pages and other composables MUST NOT call `invoke('read_tutorial_file'
// ...)` / `invoke('write_progress_file' ...)` / `invoke('list_tutorial_tasks'
// ...)` directly — `useTutorialProgress` is the only allowed caller of
// `writeProgressFile`. Defensive grep tests enforce this.

const TUTORIALS_PREFIX = 'codebus-tutorials/'

export interface TutorialTaskMeta {
  id: string
  frontmatter_raw: string | null
  dir_mtime_unix: number
}

export class TutorialFileError extends Error {
  readonly code: string
  constructor(code: string, message: string) {
    super(message)
    this.code = code
    this.name = 'TutorialFileError'
  }
}

async function tauriInvoke<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    return await invoke<T>(cmd, args)
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err)
    throw new TutorialFileError(`${cmd}_failed`, message)
  }
}

async function readTutorialFile(
  workspaceRoot: string,
  relativePath: string
): Promise<string> {
  if (!relativePath.startsWith(TUTORIALS_PREFIX)) {
    throw new TutorialFileError(
      'invalid_relative_path',
      `relativePath must start with '${TUTORIALS_PREFIX}': ${relativePath}`
    )
  }
  return tauriInvoke<string>('read_tutorial_file', {
    workspaceRoot,
    relativePath
  })
}

async function writeProgressFile(
  workspaceRoot: string,
  taskId: string,
  payload: string
): Promise<void> {
  return tauriInvoke<void>('write_progress_file', {
    workspaceRoot,
    taskId,
    payload
  })
}

async function listTutorialTasks(
  workspaceRoot: string
): Promise<TutorialTaskMeta[]> {
  return tauriInvoke<TutorialTaskMeta[]>('list_tutorial_tasks', {
    workspaceRoot
  })
}

interface TutorialFilesApi {
  readTutorialFile: typeof readTutorialFile
  writeProgressFile: typeof writeProgressFile
  listTutorialTasks: typeof listTutorialTasks
}

export function useTutorialFiles(): TutorialFilesApi {
  return {
    readTutorialFile,
    writeProgressFile,
    listTutorialTasks
  }
}
