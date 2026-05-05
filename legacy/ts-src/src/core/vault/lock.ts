import { writeFile, unlink } from 'node:fs/promises'

export interface LockHandle {
  path: string
  released: boolean
}

export async function acquireLock(lockPath: string): Promise<LockHandle> {
  try {
    await writeFile(lockPath, String(process.pid), { flag: 'wx' })
  } catch (err: unknown) {
    if ((err as NodeJS.ErrnoException).code === 'EEXIST') {
      throw new Error(`Lock already held at ${lockPath}`)
    }
    throw err
  }
  return { path: lockPath, released: false }
}

export async function releaseLock(handle: LockHandle): Promise<void> {
  if (handle.released) return
  try {
    await unlink(handle.path)
  } catch (err: unknown) {
    if ((err as NodeJS.ErrnoException).code !== 'ENOENT') throw err
  }
  handle.released = true
}
