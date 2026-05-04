export function utcTodayISO(): string {
  return new Date().toISOString().slice(0, 10)
}
