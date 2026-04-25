// Synthetic mock adapter — golden fixture stub. Not valid TS.
// Mirrors `tests/golden/timeline-gdrive-adapter/ideal-route.md` 站 2.
import type { IStorageService, TimelineConfig, EventNode, AppSettings } from '../types'

export class MockStorageAdapter implements IStorageService {
  private timelines = new Map<string, TimelineConfig>()
  private nodes = new Map<string, EventNode>()
  private settings: AppSettings = { locale: 'zh-TW' }

  async getTimeline(id: string) { return this.timelines.get(id) ?? null }
  async saveTimeline(cfg: TimelineConfig) { this.timelines.set(cfg.id, cfg) }
  async listTimelines() { return [...this.timelines.values()] }
  async deleteTimeline(id: string) { this.timelines.delete(id) }
  async getNode(id: string) { return this.nodes.get(id) ?? null }
  async saveNode(n: EventNode) { this.nodes.set(n.id, n) }
  async deleteNode(id: string) { this.nodes.delete(id) }
  async getSettings() { return this.settings }
  async saveSettings(s: AppSettings) { this.settings = s }
  async putMedia(_id: string, _b: Blob) { /* stub */ }
  async getMedia(_id: string) { return null }
  async deleteMedia(_id: string) { /* stub */ }
}
