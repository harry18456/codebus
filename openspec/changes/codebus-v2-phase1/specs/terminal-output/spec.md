## ADDED Requirements

### Requirement: Render per-event stream output with emoji or symbol prefix

The system SHALL render each StreamEvent (`thought`, `tool_use`, `tool_result`) to a single terminal line whose prefix indicates the event kind, using emoji glyphs when emoji mode is enabled and ASCII/unicode symbols when disabled.

#### Scenario: Thought event with emoji enabled

- **WHEN** the system renders a `thought` event with text `"thinking"` and emoji enabled
- **THEN** the output line begins with the thought emoji glyph and contains the text `"thinking"`

#### Scenario: Thought event with emoji disabled

- **WHEN** the system renders a `thought` event with emoji disabled
- **THEN** the output line begins with the thought symbol `"◆"` and contains the text, with no emoji glyph present

#### Scenario: tool_use Write rendered with write glyph and file path

- **WHEN** the system renders a `tool_use` event with name `"Write"` and input `{file_path: "wiki/pages/a.md"}` with emoji enabled
- **THEN** the output line begins with the write emoji glyph and contains the path `"wiki/pages/a.md"`

#### Scenario: tool_use Read rendered with tool glyph and tool name

- **WHEN** the system renders a `tool_use` event with name `"Read"` and emoji enabled
- **THEN** the output line begins with the tool emoji glyph and contains the name `"Read"`

#### Scenario: tool_result error highlights via color, not separate emoji

- **WHEN** the system renders a `tool_result` event with `isError: true` and color enabled
- **THEN** the output line uses the same result emoji glyph as success but applies red color to the result text, and the output contains the error text

### Requirement: Render lifecycle banners

The system SHALL render four banner messages (`start`, `goal`, `done`, `hint`) at appropriate lifecycle moments, with emoji or symbol prefix matching the current emoji mode.

#### Scenario: Start banner shows repo path

- **WHEN** the system renders the `start` banner with `path: "/tmp/myrepo"` and emoji enabled
- **THEN** the output contains the start emoji glyph and the path `"/tmp/myrepo"`

#### Scenario: Done banner with emoji disabled uses symbol

- **WHEN** the system renders the `done` banner with `wikiPath: ".codebus/wiki"` and emoji disabled
- **THEN** the output begins with the done symbol `"✓"` and contains the wiki path, with no emoji glyph present

### Requirement: Resolve emoji mode via 5-level priority

The system SHALL resolve the effective emoji setting by checking, in order: explicit CLI enum (`--emoji on|off|auto`), `--no-emoji` sugar (equivalent to `--emoji off`), `NO_EMOJI` env var, `~/.codebus/config.yaml emoji:` field, and finally automatic detection. Auto-detection SHALL enable emoji only when stdout is a TTY, `CI` env is unset, `NO_EMOJI` env is unset, and `TERM` is not `"dumb"`.

#### Scenario: --emoji on overrides CI auto-detect

- **WHEN** the user passes `--emoji on` while running in a CI environment
- **THEN** emoji rendering is enabled regardless of CI / TTY state

#### Scenario: --no-emoji overrides global config emoji=on

- **WHEN** `~/.codebus/config.yaml` contains `emoji: on`
- **AND** the user passes `--no-emoji`
- **THEN** emoji rendering is disabled

#### Scenario: NO_EMOJI env disables emoji when no CLI flag set

- **WHEN** no `--emoji` or `--no-emoji` flag is passed
- **AND** `NO_EMOJI=1` is set
- **THEN** emoji rendering is disabled

#### Scenario: Auto mode respects TTY and CI signals

- **WHEN** no flag, env, or config setting is present, and the resolved mode is `"auto"`
- **THEN** emoji is enabled when `process.stdout.isTTY` is truthy AND `CI` is unset AND `TERM !== "dumb"`, and disabled otherwise

### Requirement: Apply chalk color when stdout is a TTY

The system SHALL apply ANSI color via chalk only when stdout is a TTY and `NO_COLOR` env is unset; otherwise output SHALL be plain text without escape codes.

#### Scenario: Non-TTY stdout produces plain text

- **WHEN** the system renders any event with stdout redirected to a file or pipe
- **THEN** the output contains no ANSI color escape codes

#### Scenario: NO_COLOR env disables color even on TTY

- **WHEN** stdout is a TTY but `NO_COLOR` is set
- **THEN** the output contains no ANSI color escape codes

### Requirement: Load global config tolerantly

The system SHALL load `~/.codebus/config.yaml` if present, ignore unknown fields silently (forward-compat for phase 2), warn but not abort on parse errors, and warn on unknown values for known fields.

#### Scenario: Missing config returns empty config

- **WHEN** `~/.codebus/config.yaml` does not exist
- **THEN** the system returns an empty config object without error

#### Scenario: Invalid YAML warns and falls back to empty

- **WHEN** `~/.codebus/config.yaml` contains malformed YAML
- **THEN** the system writes a warning to stderr and returns an empty config object

#### Scenario: Unknown emoji value warns and is ignored

- **WHEN** `~/.codebus/config.yaml` contains `emoji: maybe`
- **THEN** the system writes a warning, and `emoji` is treated as unset (falling through to next priority level)

#### Scenario: Phase 2 fields are silently ignored

- **WHEN** `~/.codebus/config.yaml` contains `default_provider: anthropic-sdk` alongside `emoji: on`
- **THEN** the system honors `emoji: on` and silently ignores `default_provider` without warning
