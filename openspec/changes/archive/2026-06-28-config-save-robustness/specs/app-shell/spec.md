## ADDED Requirements

### Requirement: Config Save Hygiene

The `save_global_config` IPC and the Settings UI SHALL keep empty patterns out of persisted config and SHALL keep the saved file's shape consistent with the CLI-written starter.

Before writing, `save_global_config` SHALL drop every `pii.patterns_extra` entry that is empty or whitespace-only, so an empty pattern never lands on disk. `save_global_config` SHALL prepend the shared `CONFIG_HEADER` — the same source-of-truth constant the CLI starter uses (see the `cli` capability's Global Config Starter Content Shape requirement) — to the serialized YAML before the atomic write, so an app-saved config and a freshly-written starter share the same header-plus-pure-values shape. Prepending the header SHALL NOT change how the file parses on the next load, because the header is a YAML comment.

The Settings UI SHALL omit empty or whitespace-only `pii.patterns_extra` entries from the payload it sends to `save_global_config`, so a blank rule the user added but never filled in is not sent for persistence. Adding a blank row in the editor SHALL remain allowed so the user can type into it; only persistence SHALL filter blanks.

#### Scenario: Empty extra pattern is not persisted

- **WHEN** `save_global_config` receives a payload whose `pii.patterns_extra` is `["", "real-pattern"]`
- **THEN** the written file's `pii.patterns_extra` SHALL contain only `real-pattern` AND a subsequent `load_global_config` SHALL return only `["real-pattern"]`

#### Scenario: Saved config carries the shared header

- **WHEN** `save_global_config` writes a payload to disk
- **THEN** the written YAML SHALL begin with the shared `CONFIG_HEADER` block AND SHALL load back without error

#### Scenario: Settings UI drops blank pattern rows on save

- **WHEN** the user adds a blank PII extra-pattern row and clicks Save without filling it in
- **THEN** the payload sent to `save_global_config` SHALL NOT include the blank entry
