// Synthetic local-file adapter — golden fixture stub. Not valid TS.
// Mirrors `tests/golden/timeline-gdrive-adapter/ideal-route.md` 站 3.
import type { IStorageService, TimelineConfig, EventNode, AppSettings } from '../types'

export class LocalFileAdapter implements IStorageService {
  private rootHandle: any = null // File handle init — set by showDirectoryPicker
  private observer: any = null   // FileSystemObserver stub

  async init(handle: any) {
    this.rootHandle = handle
    // change detection — real impl wires FileSystemObserver here
    this.observer = { observe: () => undefined }
  }

  async getTimeline(_id: string) { return null /* read timeline.json */ }
  async saveTimeline(_cfg: TimelineConfig) { /* write timeline.json */ }
  async listTimelines() { return [] }
  async deleteTimeline(_id: string) { /* unlink timeline file */ }
  async getNode(_id: string) { return null /* read nodes/{id}.md */ }
  async saveNode(_n: EventNode) { /* write nodes/{id}.md */ }
  async deleteNode(_id: string) { /* unlink node file */ }
  async getSettings() { return { locale: 'zh-TW' } as AppSettings }
  async saveSettings(_s: AppSettings) { /* write settings.json */ }
  async putMedia(_id: string, _b: Blob) { /* binary write */ }
  async getMedia(_id: string) { return null }
  async deleteMedia(_id: string) { /* unlink media */ }
}
