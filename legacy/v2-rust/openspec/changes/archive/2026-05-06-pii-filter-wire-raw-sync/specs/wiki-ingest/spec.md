## ADDED Requirements

### Requirement: --goal flow's raw_sync invokes the configured PII scanner

During the raw_sync stage of the `--goal` flow, the system SHALL invoke the PII scanner that has been built from `~/.codebus/config.yaml` `pii` section (via the PII factory) before each candidate text file is mirrored. The scanner instance and its `on_hit` mode SHALL be supplied by the goal command from the loaded global config; default config (no `pii` section, or `pii.scanner: null`) SHALL select the no-op `NullScanner` so existing 0.2.0 raw mirror behavior is preserved.

#### Scenario: Goal command propagates PII config from global config to raw_sync

- **WHEN** the user runs `codebus --repo X --goal "Y"`
- **AND** `~/.codebus/config.yaml` contains `pii: { scanner: regex_basic, on_hit: warn }`
- **THEN** the goal command builds a `RegexBasicScanner` via the PII factory using the loaded `pii` config
- **AND** raw_sync receives that scanner instance with `OnHit::Warn`

#### Scenario: Default config preserves 0.2.0 behavior in goal flow

- **WHEN** `~/.codebus/config.yaml` does not exist or has no `pii` section
- **AND** the user runs `codebus --repo X --goal "Y"`
- **THEN** the goal command supplies a `NullScanner` to raw_sync
- **AND** raw_sync mirrors every text file byte-for-byte regardless of content

#### Scenario: Scanner construction failure aborts the goal

- **WHEN** `~/.codebus/config.yaml` contains `pii.patterns_extra` with a malformed regex such as `[unterminated`
- **AND** the user runs `codebus --repo X --goal "Y"`
- **THEN** the system writes a user-facing error to stderr identifying the unbuildable scanner
- **AND** the process exits with a non-zero exit code BEFORE invoking the LLM agent
