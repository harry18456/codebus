// Synthetic Storage Adapter interface — golden fixture stub.
// Mirrors `tests/golden/timeline-gdrive-adapter/ideal-route.md` 站 1.
// Not valid TypeScript — fixture is read by grep, never compiled.

export interface TimelineConfig { id: string; name: string }
export interface EventNode { id: string; title: string }
export interface AppSettings { locale: string }

export interface IStorageService {
  getTimeline(id: string): Promise<TimelineConfig | null>
  saveTimeline(cfg: TimelineConfig): Promise<void>
  listTimelines(): Promise<TimelineConfig[]>
  deleteTimeline(id: string): Promise<void>
  getNode(id: string): Promise<EventNode | null>
  saveNode(node: EventNode): Promise<void>
  deleteNode(id: string): Promise<void>
  getSettings(): Promise<AppSettings>
  saveSettings(s: AppSettings): Promise<void>
  putMedia(id: string, blob: Blob): Promise<void>
  getMedia(id: string): Promise<Blob | null>
  deleteMedia(id: string): Promise<void>
}
