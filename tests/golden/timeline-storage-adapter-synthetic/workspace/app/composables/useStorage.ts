// Synthetic Storage composable — golden fixture stub. Not valid TS.
// Mirrors `tests/golden/timeline-gdrive-adapter/ideal-route.md` 站 4.
import { MockStorageAdapter } from '../services/MockStorageAdapter'
import { LocalFileAdapter } from '../services/LocalFileAdapter'
import type { IStorageService } from '../types'

let $storage: IStorageService | null = null
let $storageReady = false

export function useStorage() {
  return {
    $storage,
    $storageReady,
    $initStorage,
    $changeFolder,
  }
}

async function $initStorage(mode: 'mock' | 'local') {
  $storage = mode === 'mock'
    ? new MockStorageAdapter()
    : new LocalFileAdapter()
  $storageReady = true
}

async function $changeFolder() {
  // re-prompt user for new directory handle
  $storageReady = false
  await $initStorage('local')
}
