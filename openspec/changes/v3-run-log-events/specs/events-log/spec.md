## ADDED Requirements

### Requirement: EventsSink Trait and Implementations

The system SHALL define an object-safe `EventsSink` trait in `codebus_core::log::events::sink` with three methods: `name(&self) -> &str` returning a stable identifier, `write_event(&mut self, envelope: &EventEnvelope) -> Result<(), LogError>` persisting one event envelope, and `flush(&mut self) -> Result<(), LogError>` with a default no-op body for sinks without internal buffers. The trait SHALL be `Send + Sync` so a sink instance can outlive a verb invocation if a future daemon mode keeps one process across multiple runs.

The system SHALL provide two implementations:

- **EventsNullSink** — stable name `"null"`. `write_event` SHALL return `Ok(())` without I/O. Used as the user-facing opt-out path that mirrors the `LogSink::NullSink` opt-out behavior.
- **EventsJsonlSink** — stable name `"jsonl"`. `write_event` SHALL append the serialized envelope plus a trailing newline byte to `<dir>/events-<slug>.jsonl`, where `<dir>` is the directory passed at construction and `<slug>` is the run's `started_at` RFC 3339 string with each `:` character replaced by `-` (Windows filename compatibility). The sink SHALL `create_dir_all(<dir>)` lazily on first `write_event` call. The file SHALL be opened with `OpenOptions::new().append(true).create(true)` so concurrent writes from different processes are line-wise atomic on POSIX (best-effort on Windows). Each call SHALL `flush()` after writing to ensure bytes reach OS page cache for crash resilience.

The `EventsSink` trait is intentionally separate from the existing `LogSink` trait — the two have different lifecycles (per-run summary vs. per-event live append) and different optimal default behaviors.

#### Scenario: EventsNullSink write_event is a successful no-op

- **WHEN** the caller invokes `EventsNullSink::new().write_event(&envelope)`
- **THEN** the call SHALL return `Ok(())` AND no filesystem state SHALL change

#### Scenario: EventsJsonlSink writes one JSON line plus newline

- **WHEN** `EventsJsonlSink::new(dir, "2026-05-13T03:25:11Z").write_event(&envelope)` is called
- **THEN** the file `<dir>/events-2026-05-13T03-25-11Z.jsonl` SHALL exist AND its contents SHALL end with one valid JSON line for the envelope followed by `\n`

#### Scenario: EventsJsonlSink slug replaces every colon with dash

- **WHEN** `EventsJsonlSink::new(dir, "2026-05-13T23:55:00Z")` is constructed
- **THEN** the target filename SHALL equal `events-2026-05-13T23-55-00Z.jsonl` (every `:` replaced) AND the filename SHALL contain zero `:` characters (verified by character scan)

#### Scenario: EventsJsonlSink appends multiple events to the same file

- **WHEN** `EventsJsonlSink::new(dir, "2026-05-13T03:25:11Z")` is constructed AND `write_event` is called 5 times with distinct envelopes
- **THEN** the target file SHALL contain exactly 5 lines AND each line SHALL parse as JSON containing both `ts` and `event` fields

#### Scenario: EventsJsonlSink creates directory lazily on first write

- **WHEN** `EventsJsonlSink::new("/tmp/no/such/dir", started_at).write_event(&envelope)` is called and `/tmp/no/such/dir/` does not yet exist
- **THEN** the directory SHALL be created via `create_dir_all` AND the write SHALL succeed

#### Scenario: EventsJsonlSink flushes per write for crash resilience

- **WHEN** `EventsJsonlSink::write_event` returns `Ok(())`
- **THEN** the BufWriter underlying the file handle SHALL have been flushed exactly once during the call AND a subsequent process crash before the next `write_event` SHALL still leave the previously-written envelope readable from disk

---
### Requirement: EventEnvelope Schema

The system SHALL define `EventEnvelope` in `codebus_core::log::events::sink` as a public struct containing exactly two fields:

```
pub struct EventEnvelope {
    pub ts: String,         // RFC 3339 UTC wall-clock timestamp at append time
    pub event: VerbEvent,   // serialized VerbEvent from codebus_core::verb
}
```

The `ts` field SHALL be captured by the verb function immediately before invoking `EventsSink::write_event`, using `chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)` (matching the existing `started_at` / `finished_at` timestamp format throughout the codebase). The `event` field SHALL be the serde-serialized `VerbEvent` (which already derives `Serialize` via the `agent-stream-rendering` / `verb-library` capabilities).

The serialized envelope SHALL emit JSON with both top-level keys `ts` and `event` present on every line. The `event` value SHALL be a JSON object whose shape matches the `VerbEvent` enum's serde-tagged representation (`{"kind": "banner"|"stream"|"lifecycle", ...}`).

All three `VerbEvent` variants (`Banner(VerbBanner)`, `Stream(StreamEvent)`, `Lifecycle(VerbLifecycleEvent)`) SHALL be serialized — none filtered. This preserves the full timeline including banner milestone messages (e.g., `"sync_done"` with file count and elapsed time) and lifecycle hooks (e.g., `"spawn_start"` / `"spawn_end"` with exit code) that future GUI views and analytics consumers need.

#### Scenario: EventEnvelope serializes with ts and event keys

- **WHEN** an `EventEnvelope { ts: "2026-05-13T03:25:11Z", event: VerbEvent::Stream(StreamEvent::Thought { text: "hi" }) }` is serialized via `serde_json::to_string`
- **THEN** the resulting JSON string SHALL contain `"ts":"2026-05-13T03:25:11Z"` AND `"event":{"kind":"stream",...}` AND parse cleanly via `serde_json::from_str::<EventEnvelope>`

#### Scenario: All three VerbEvent variants are persisted

- **WHEN** a verb function emits a `VerbEvent::Banner`, a `VerbEvent::Stream`, AND a `VerbEvent::Lifecycle` during one run
- **THEN** the resulting events.jsonl file SHALL contain three lines (in emit order) AND each line SHALL parse as `EventEnvelope` AND the `event.kind` values SHALL match `"banner"`, `"stream"`, `"lifecycle"` respectively

---
### Requirement: build_events_sink Factory

The system SHALL define `build_events_sink(cfg: &SinkConfig, started_at: &str) -> Result<Box<dyn EventsSink>, SinkError>` in `codebus_core::log::factory`. The factory SHALL dispatch on the same `SinkConfig` discriminator used by the existing `build_sink` (`Null` vs `Jsonl { dir }`) so that the user-facing `log.sink` yaml configuration controls both runs.jsonl and events.jsonl together.

- `SinkConfig::Null {}` → `EventsNullSink` instance with `name() == "null"`
- `SinkConfig::Jsonl { dir: Some(path) }` → `EventsJsonlSink::new(path, started_at)` instance with `name() == "jsonl"`
- `SinkConfig::Jsonl { dir: None }` → `Err(SinkError::Setup(...))` whose message references the `dir` field (mirrors `build_sink`'s precondition)

The factory SHALL be the only public constructor for `Box<dyn EventsSink>` so callers cannot accidentally bypass the dir-resolution requirement.

#### Scenario: build_events_sink dispatches to Null and Jsonl correctly

- **WHEN** `build_events_sink(&SinkConfig::Null {}, "2026-05-13T03:25:11Z")` is called
- **THEN** the returned trait object's `name()` SHALL equal `"null"`

- **WHEN** `build_events_sink(&SinkConfig::Jsonl { dir: Some(tmp_dir) }, "2026-05-13T03:25:11Z")` is called with a real path
- **THEN** the returned trait object's `name()` SHALL equal `"jsonl"`

#### Scenario: build_events_sink rejects Jsonl with unresolved dir

- **WHEN** `build_events_sink(&SinkConfig::Jsonl { dir: None }, started_at)` is called
- **THEN** the result SHALL be `Err(SinkError::Setup(_))` whose message references the `dir` field

---
### Requirement: log.sink Discriminator Shared with Runs Sink

The system SHALL honor the `log.sink` yaml discriminator as the single switch controlling both the runs.jsonl `LogSink` AND the events.jsonl `EventsSink`. When `log.sink: jsonl` (or absent — default), both sinks SHALL be active and write to the same resolved directory (`<vault>/.codebus/log/` by default, override via `log.dir`). When `log.sink: none`, both sinks SHALL be opt-out NullSinks and no log files SHALL be written by either path.

No separate yaml field (`log.events_sink`, `log.events_dir`, etc.) SHALL be introduced. The intent is one user-visible knob covering all logging, so a CLI user who runs `codebus query` after editing `log.sink: none` gets a clean run with zero log files on disk.

#### Scenario: log.sink none disables both runs and events log

- **WHEN** `~/.codebus/config.yaml` contains `log:\n  sink: none\n` AND a verb (`goal` / `query` / `fix`) runs to completion
- **THEN** the `<vault>/.codebus/log/` directory SHALL contain zero new files for that invocation (no `runs-YYYY-MM-DD.jsonl` row appended AND no `events-<slug>.jsonl` file created)

#### Scenario: log.sink jsonl enables both runs and events log

- **WHEN** `~/.codebus/config.yaml` is missing OR contains `log:\n  sink: jsonl\n` AND a verb runs to completion
- **THEN** `<vault>/.codebus/log/runs-YYYY-MM-DD.jsonl` SHALL have one new row appended AND `<vault>/.codebus/log/events-<slug>.jsonl` SHALL exist with at least one envelope row

---
### Requirement: events.jsonl Write Failure Is Non-Fatal

When `EventsSink::write_event` returns `Err(LogError::*)` during a verb run, the verb SHALL emit a stderr warning prefixed with `warning: events-log` describing the error and SHALL continue the run path normally (the warning SHALL NOT propagate into the verb's exit code or return error). This mirrors the existing `RunLog Write Failure Is Non-Fatal` requirement from the `run-log` capability — logging is best-effort; a disk-full / permission-denied / locked-file failure on the events.jsonl path MUST NOT fail an otherwise successful agent run.

When `build_events_sink` returns `Err(SinkError::*)` at the verb function entry, the verb SHALL emit a stderr warning prefixed with `warning: events-log sink build failed (skipping persistence):` and SHALL continue the run with a no-op fallback (subsequent `write_event` calls SHALL be skipped silently for the remainder of that verb invocation).

#### Scenario: EventsJsonlSink IO error becomes warning, exit code unchanged

- **WHEN** a verb runs to a successful agent termination AND the configured `EventsJsonlSink` cannot write (e.g., the target directory's parent is read-only)
- **THEN** stderr SHALL contain at least one line beginning with `warning: events-log` AND the verb's exit code SHALL be 0 (not 1)

#### Scenario: build_events_sink failure leaves verb running

- **WHEN** `build_events_sink` is called with `SinkConfig::Jsonl { dir: None }` AND the verb function consequently observes `Err(SinkError::Setup(_))`
- **THEN** stderr SHALL contain a line beginning with `warning: events-log sink build failed` AND the verb SHALL continue execution with a fallback no-op events sink AND no events.jsonl file SHALL be created
