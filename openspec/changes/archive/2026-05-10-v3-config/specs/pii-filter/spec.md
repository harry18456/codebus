## MODIFIED Requirements

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

## ADDED Requirements

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
