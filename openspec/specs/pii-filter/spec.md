# pii-filter Specification

## Purpose

The PII detection scanner trait surface used by `vault`'s raw-mirror sync — the `PiiScanner` interface, match output shape (`pattern_name` identifier, `severity` classification of `Critical`/`Warn`, byte-offset ordering), built-in `RegexBasicScanner` with credential / contact / network pattern coverage, `NullScanner` for tests and opt-out flows, and the warn-by-default on-hit policy (matches surface as warnings; mirroring still proceeds). Does NOT cover the file-walk / mirror logic itself (lives in `vault` Raw Mirror with PII Scanner), nor any Obsidian / vault metadata processing.

## Requirements

### Requirement: PII Match Output Shape

The system SHALL classify every PII match with a stable `pattern_name` string identifier and a `severity` value. The `severity` SHALL be one of two closed values: `Critical` (definitely sensitive: secrets, API keys, credentials) or `Warn` (probably sensitive: emails, IP addresses). The system SHALL return matches sorted in ascending byte-offset order regardless of pattern definition order.

#### Scenario: Critical severity assigned to credential patterns

- **WHEN** a scanner reports a match for an AWS access key shape or an Anthropic API key shape
- **THEN** the resulting match SHALL carry `severity` equal to `Critical`

#### Scenario: Warn severity assigned to ambiguous PII

- **WHEN** a scanner reports a match for an email address or an IPv4 address
- **THEN** the resulting match SHALL carry `severity` equal to `Warn`

#### Scenario: Mixed pattern matches sorted by byte offset

- **WHEN** the scanner produces multiple matches across different patterns within one input
- **THEN** the returned match list SHALL be sorted in ascending order by the match start byte offset


<!-- @trace
source: v3-pii
updated: 2026-05-09
code:
  - codebus-core/src/pii/scanners/null_scanner.rs
  - codebus-core/src/pii/scanners/mod.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/vault/raw_sync.rs
  - codebus-core/src/lib.rs
  - codebus-core/Cargo.toml
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/pii/provider.rs
  - codebus-core/src/pii/mod.rs
  - codebus-core/src/pii/scanners/regex_basic.rs
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-core/tests/vault_init.rs
-->

---
### Requirement: Built-in Regex Pattern Coverage

The system SHALL provide a default regex-based scanner that detects four PII categories with stable pattern names: `aws-access-key` (token starting with `AKIA` or `ASIA` followed by exactly 16 uppercase alphanumerics), `anthropic-api-key` (token starting with `sk-ant-` followed by 20 or more URL-safe characters), `email` (RFC 5322-ish ASCII local part, an `@`, a domain containing at least one dot, and a TLD of two or more letters), and `ipv4` (four dot-separated digit groups). The system SHALL compile all built-in patterns once at scanner construction; user-supplied extra patterns SHALL be rejected at construction if any fails to compile.

#### Scenario: Detects AWS access key

- **WHEN** the scanner is given content containing the substring `AKIAIOSFODNN7EXAMPLE`
- **THEN** the resulting match list SHALL contain exactly one entry whose `pattern_name` equals `aws-access-key` and whose `matched_text` equals `AKIAIOSFODNN7EXAMPLE`

#### Scenario: Detects Anthropic API key

- **WHEN** the scanner is given content containing `sk-ant-api01-abcDEF123456789_-XYZ012345`
- **THEN** the resulting match list SHALL contain exactly one entry whose `pattern_name` equals `anthropic-api-key`

#### Scenario: Detects email address

- **WHEN** the scanner is given content containing `alice@example.com`
- **THEN** the resulting match list SHALL contain exactly one entry whose `pattern_name` equals `email` and whose `matched_text` equals `alice@example.com`

#### Scenario: Detects IPv4 address

- **WHEN** the scanner is given content containing `192.168.1.42`
- **THEN** the resulting match list SHALL contain exactly one entry whose `pattern_name` equals `ipv4` and whose `matched_text` equals `192.168.1.42`

#### Scenario: Ignores AWS lookalike with insufficient trailing length

- **WHEN** the scanner is given content containing `AKIA12345ABCDEFGH` (15 trailing alphanumerics, not 16)
- **THEN** the resulting match list SHALL NOT contain any entry with `pattern_name` equal to `aws-access-key`

#### Scenario: Ignores email without TLD

- **WHEN** the scanner is given content containing `user@localhost` (no dot in domain)
- **THEN** the resulting match list SHALL NOT contain any entry with `pattern_name` equal to `email`

#### Scenario: Ignores version string lookalike

- **WHEN** the scanner is given content containing `v1.2.3` (only three dot-separated groups)
- **THEN** the resulting match list SHALL NOT contain any entry with `pattern_name` equal to `ipv4`

#### Scenario: Malformed extra pattern fails fast at construction

- **WHEN** the scanner is constructed with an extra regex source that fails to compile
- **THEN** construction SHALL return an error AND the scanner instance SHALL NOT be returned


<!-- @trace
source: v3-pii
updated: 2026-05-09
code:
  - codebus-core/src/pii/scanners/null_scanner.rs
  - codebus-core/src/pii/scanners/mod.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/vault/raw_sync.rs
  - codebus-core/src/lib.rs
  - codebus-core/Cargo.toml
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/pii/provider.rs
  - codebus-core/src/pii/mod.rs
  - codebus-core/src/pii/scanners/regex_basic.rs
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-core/tests/vault_init.rs
-->

---
### Requirement: Null Scanner Behavior

The system SHALL provide a no-op scanner whose `scan` method returns an empty match list for any input regardless of input content.

#### Scenario: Null scanner returns empty for clean input

- **WHEN** the null scanner scans the content `hello world`
- **THEN** the returned match list SHALL be empty

#### Scenario: Null scanner returns empty even when input contains secret-shaped tokens

- **WHEN** the null scanner scans content containing the substring `AKIAIOSFODNN7EXAMPLE`
- **THEN** the returned match list SHALL be empty


<!-- @trace
source: v3-pii
updated: 2026-05-09
code:
  - codebus-core/src/pii/scanners/null_scanner.rs
  - codebus-core/src/pii/scanners/mod.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/vault/raw_sync.rs
  - codebus-core/src/lib.rs
  - codebus-core/Cargo.toml
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/pii/provider.rs
  - codebus-core/src/pii/mod.rs
  - codebus-core/src/pii/scanners/regex_basic.rs
tests:
  - codebus-cli/tests/cli_routing.rs
  - codebus-core/tests/vault_init.rs
-->

---
### Requirement: On-Hit Policy Default

The system SHALL define three on-hit policy values: `Warn`, `Skip`, and `Mask`. The system SHALL select the active on-hit policy from the `pii.on_hit` field in `~/.codebus/config.yaml`. The default value SHALL be `Mask` when the config file is missing, the `pii` section is absent, or the `on_hit` field is absent. The system SHALL implement the runtime behavior of all three policy values (no value SHALL be deferred or unimplemented).

#### Scenario: Default on-hit policy value is Mask

- **WHEN** the config file is absent
- **THEN** the resulting on-hit policy value SHALL equal `Mask`

#### Scenario: Configuration overrides default on-hit selection

- **WHEN** `~/.codebus/config.yaml` contains `pii.on_hit: warn`
- **THEN** the resulting on-hit policy value SHALL equal `Warn`

#### Scenario: Unknown on-hit value falls back to default

- **WHEN** `~/.codebus/config.yaml` contains `pii.on_hit: hyperflood` (an unknown discriminator)
- **THEN** the loader SHALL return a parse error AND the caller SHALL fall back to the default `Mask` after emitting a stderr warning prefixed with `warning: pii config`


<!-- @trace
source: v3-config
updated: 2026-05-10
code:
  - codebus-core/src/config/global_starter.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/config/lint_fix.rs
  - codebus-cli/src/commands/query.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-core/src/config/mod.rs
  - codebus-cli/src/commands/init.rs
  - codebus-cli/src/commands/fix.rs
  - codebus-core/src/vault/raw_sync.rs
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/config/pii.rs
tests:
  - codebus-cli/tests/query_flow.rs
  - codebus-cli/tests/cli_routing.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/fix_flow.rs
  - codebus-cli/tests/lint_flow.rs
  - codebus-core/tests/vault_init.rs
-->

---
### Requirement: PII Configuration Schema

The system SHALL load PII configuration from `~/.codebus/config.yaml` under the top-level key `pii`. The schema SHALL define exactly three optional fields: `scanner` (string discriminator with values `regex_basic` or `none`, default `regex_basic`; the literal value `null` SHALL be rejected by the wire format because YAML treats it as the null literal which would silently collapse to "field absent"), `patterns_extra` (list of regex source strings, default empty list), and `on_hit` (string discriminator with values `warn`, `skip`, or `mask`, default `mask`). When the file is missing, the `pii` section is absent, or any individual field is absent, the system SHALL apply the field's default value. Unknown keys inside the `pii` section SHALL be silently ignored to preserve forward-compatibility.

#### Scenario: Default config when file missing

- **WHEN** `~/.codebus/config.yaml` does not exist
- **THEN** the loaded `PiiConfig` SHALL equal `{ scanner: regex_basic, patterns_extra: [], on_hit: mask }` AND no stderr message SHALL be emitted

#### Scenario: Default config when pii section absent

- **WHEN** `~/.codebus/config.yaml` exists with content `lint:\n  fix:\n    enabled: true\n` and no `pii:` key
- **THEN** the loaded `PiiConfig` SHALL equal `{ scanner: regex_basic, patterns_extra: [], on_hit: mask }`

#### Scenario: Partial config fills missing fields with defaults

- **WHEN** `~/.codebus/config.yaml` contains `pii:\n  scanner: none\n` and no other `pii.*` keys
- **THEN** the loaded `PiiConfig` SHALL equal `{ scanner: Null, patterns_extra: [], on_hit: mask }`

#### Scenario: YAML null literal as scanner value falls through to default

- **WHEN** `~/.codebus/config.yaml` contains `pii:\n  scanner: null\n` (the bare YAML null literal, NOT the string `none`)
- **THEN** the YAML parser SHALL collapse the null literal to "field absent" AND the loaded `PiiConfig.scanner` SHALL equal `RegexBasic` (the default), preventing the silent foot-gun of users believing they had disabled scanning

#### Scenario: Unknown pii subkey silently ignored

- **WHEN** `~/.codebus/config.yaml` contains `pii:\n  future_field: hello\n  scanner: regex_basic\n`
- **THEN** the loader SHALL succeed AND the resulting `scanner` SHALL equal `regex_basic` AND the unknown `future_field` SHALL have no observable effect


<!-- @trace
source: v3-config
updated: 2026-05-10
code:
  - codebus-core/src/config/global_starter.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/config/lint_fix.rs
  - codebus-cli/src/commands/query.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-core/src/config/mod.rs
  - codebus-cli/src/commands/init.rs
  - codebus-cli/src/commands/fix.rs
  - codebus-core/src/vault/raw_sync.rs
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/config/pii.rs
tests:
  - codebus-cli/tests/query_flow.rs
  - codebus-cli/tests/cli_routing.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/fix_flow.rs
  - codebus-cli/tests/lint_flow.rs
  - codebus-core/tests/vault_init.rs
-->

---
### Requirement: Scanner Selection from Config

The system SHALL select the active PII scanner implementation based on the `pii.scanner` config field. When the value is `regex_basic`, the system SHALL construct a `RegexBasicScanner` whose pattern set is the four built-in patterns plus any entries from `pii.patterns_extra`. When the value is `none`, the system SHALL construct a `NullScanner` and SHALL NOT compile any regex from `pii.patterns_extra`.

#### Scenario: regex_basic scanner is constructed when configured

- **WHEN** `pii.scanner: regex_basic` is loaded from config
- **THEN** the constructed scanner SHALL be a `RegexBasicScanner` instance

#### Scenario: none scanner is constructed when configured

- **WHEN** `pii.scanner: none` is loaded from config
- **THEN** the constructed scanner SHALL be a `NullScanner` instance AND the `patterns_extra` field SHALL NOT be evaluated for compilation

#### Scenario: patterns_extra regex compile failure falls back to built-in only

- **WHEN** `pii.scanner: regex_basic` and `pii.patterns_extra` contains an entry that fails to compile as a regex
- **THEN** the system SHALL emit a stderr warning prefixed with `warning: pii config` AND SHALL construct the `RegexBasicScanner` with the built-in pattern set only (zero extra patterns) AND SHALL NOT abort the surrounding operation


<!-- @trace
source: v3-config
updated: 2026-05-10
code:
  - codebus-core/src/config/global_starter.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/config/lint_fix.rs
  - codebus-cli/src/commands/query.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-core/src/config/mod.rs
  - codebus-cli/src/commands/init.rs
  - codebus-cli/src/commands/fix.rs
  - codebus-core/src/vault/raw_sync.rs
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/config/pii.rs
tests:
  - codebus-cli/tests/query_flow.rs
  - codebus-cli/tests/cli_routing.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/fix_flow.rs
  - codebus-cli/tests/lint_flow.rs
  - codebus-core/tests/vault_init.rs
-->

---
### Requirement: Mirror Skip Behavior

When the active on-hit policy is `Skip` and a scanned file produces one or more PII matches, the system SHALL NOT copy the file to the raw mirror destination. The system SHALL still emit one warning line per match to the warn sink (so the user knows which files were skipped). The system SHALL increment a `pii_skipped_files` counter on the sync summary by exactly one per skipped file (regardless of how many matches that file produced).

#### Scenario: File with matches is omitted from mirror under Skip

- **WHEN** the raw-mirror sync runs with on-hit policy `Skip` against a source file containing exactly one AWS access key match
- **THEN** the destination path under the raw mirror directory SHALL NOT exist after the sync completes AND the warn sink SHALL contain at least one line beginning with `pii warn: aws-access-key`

#### Scenario: Files without matches are mirrored normally under Skip

- **WHEN** the raw-mirror sync runs with on-hit policy `Skip` against a source file with zero matches
- **THEN** the destination path under the raw mirror directory SHALL exist with byte-identical content to the source

#### Scenario: Sync summary records skipped file count

- **WHEN** the raw-mirror sync runs with on-hit policy `Skip` against three source files of which two contain at least one PII match each
- **THEN** the resulting `SyncSummary.pii_skipped_files` SHALL equal 2 AND `SyncSummary.files` SHALL equal 1 (only the clean file was mirrored)


<!-- @trace
source: v3-config
updated: 2026-05-10
code:
  - codebus-core/src/config/global_starter.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/config/lint_fix.rs
  - codebus-cli/src/commands/query.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-core/src/config/mod.rs
  - codebus-cli/src/commands/init.rs
  - codebus-cli/src/commands/fix.rs
  - codebus-core/src/vault/raw_sync.rs
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/config/pii.rs
tests:
  - codebus-cli/tests/query_flow.rs
  - codebus-cli/tests/cli_routing.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/fix_flow.rs
  - codebus-cli/tests/lint_flow.rs
  - codebus-core/tests/vault_init.rs
-->

---
### Requirement: Mirror Mask Behavior

When the active on-hit policy is `Mask` and a scanned file's content is valid UTF-8 with one or more PII matches, the system SHALL write a transformed copy of the content to the raw mirror destination in which each `matched_text` substring is replaced by the literal string `[REDACTED:<pattern_name>]`. Replacement SHALL preserve all non-matched bytes and SHALL be performed in descending byte-offset order so earlier replacements do not shift later match offsets. The warn sink SHALL still receive one line per original match. The system SHALL increment a `pii_masked_matches` counter on the sync summary by exactly one per replaced match.

#### Scenario: Match substring replaced with redaction marker

- **WHEN** the raw-mirror sync runs with on-hit policy `Mask` against a source file whose content is `pre AKIAIOSFODNN7EXAMPLE post`
- **THEN** the destination file content SHALL equal `pre [REDACTED:aws-access-key] post`

#### Scenario: Multiple matches replaced in descending offset order

- **WHEN** the raw-mirror sync runs with on-hit policy `Mask` against a source file whose content contains two matches at byte offsets 10 and 50
- **THEN** the destination file SHALL contain both replaced markers AND the byte content between offset 0 and the first replacement SHALL be byte-identical to the source

##### Example: two matches in one file

- **GIVEN** source content `start AKIAIOSFODNN7EXAMPLE middle alice@example.com end`
- **WHEN** sync runs with `on_hit: mask`
- **THEN** destination content SHALL equal `start [REDACTED:aws-access-key] middle [REDACTED:email] end`

#### Scenario: Non-UTF-8 file falls through to verbatim copy under Mask

- **WHEN** the raw-mirror sync runs with on-hit policy `Mask` against a source file whose bytes are not valid UTF-8
- **THEN** the destination file SHALL be a byte-identical copy of the source AND no warn line SHALL be emitted (the regex scanner does not produce matches against non-UTF-8 input)

#### Scenario: Sync summary records masked match count

- **WHEN** the raw-mirror sync runs with on-hit policy `Mask` against a source file containing exactly three PII matches
- **THEN** the resulting `SyncSummary.pii_masked_matches` SHALL equal 3 AND `SyncSummary.files` SHALL equal 1

<!-- @trace
source: v3-config
updated: 2026-05-10
code:
  - codebus-core/src/config/global_starter.rs
  - codebus-core/src/vault/manifest.rs
  - codebus-core/src/config/lint_fix.rs
  - codebus-cli/src/commands/query.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-core/src/config/mod.rs
  - codebus-cli/src/commands/init.rs
  - codebus-cli/src/commands/fix.rs
  - codebus-core/src/vault/raw_sync.rs
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/config/pii.rs
tests:
  - codebus-cli/tests/query_flow.rs
  - codebus-cli/tests/cli_routing.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/fix_flow.rs
  - codebus-cli/tests/lint_flow.rs
  - codebus-core/tests/vault_init.rs
-->