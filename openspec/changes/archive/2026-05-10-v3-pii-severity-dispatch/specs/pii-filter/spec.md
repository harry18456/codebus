## MODIFIED Requirements

### Requirement: On-Hit Policy Default

The system SHALL define three on-hit policy values: `Warn`, `Skip`, and `Mask`. The `pii.on_hit` field in `~/.codebus/config.yaml` SHALL select the active policy ONLY for `PiiSeverity::Warn` matches (e.g., `email`, `ipv4`). `PiiSeverity::Critical` matches (e.g., `aws-access-key`, `anthropic-api-key`) SHALL ALWAYS be processed under the `Mask` policy regardless of `pii.on_hit` configuration — this is a non-negotiable security floor: real credentials SHALL NOT enter the raw mirror in a recoverable form. The default `pii.on_hit` value SHALL be `Warn` when the config file is missing, the `pii` section is absent, or the `on_hit` field is absent.

#### Scenario: Default on-hit policy value is Warn

- **WHEN** the config file is absent
- **THEN** the resulting on-hit policy value SHALL equal `Warn` (Warn-severity matches surface as warnings only; Critical-severity matches still SHALL be masked per the security floor)

#### Scenario: Configuration overrides default Warn-severity policy

- **WHEN** `~/.codebus/config.yaml` contains `pii.on_hit: mask`
- **THEN** the resulting on-hit policy for Warn-severity matches SHALL equal `Mask` (matches both Critical AND Warn get masked — the legacy v3-config behavior remains reachable via explicit opt-in)

#### Scenario: Critical severity ignores on_hit configuration

- **WHEN** `~/.codebus/config.yaml` contains `pii.on_hit: warn` AND a source file contains an AWS access key (Critical severity)
- **THEN** the destination raw mirror file SHALL contain `[REDACTED:aws-access-key]` substituted for the matched substring (Mask behavior applied), regardless of the `warn` configuration

#### Scenario: Unknown on-hit value falls back to default

- **WHEN** `~/.codebus/config.yaml` contains `pii.on_hit: hyperflood` (an unknown discriminator)
- **THEN** the loader SHALL return a parse error AND the caller SHALL fall back to the default `Warn` after emitting a stderr warning prefixed with `warning: pii config`

### Requirement: Mirror Skip Behavior

When the active on-hit policy is `Skip` and a scanned file produces one or more `PiiSeverity::Warn` matches, the system SHALL NOT copy the file to the raw mirror destination. Files producing only `PiiSeverity::Critical` matches SHALL ALWAYS be processed under the `Mask` policy (per the `On-Hit Policy Default` security floor) — they SHALL be mirrored with each Critical match's substring replaced by `[REDACTED:<pattern_name>]`, never skipped. Files containing both Critical and Warn matches under `Skip` policy SHALL be processed under `Mask` (the Critical floor wins over the Warn-targeted Skip request to avoid silently dropping files that have any credential exposure). The system SHALL still emit one warning line per match to the warn sink (so the user knows which files were skipped or masked). The system SHALL increment a `pii_skipped_files` counter on the sync summary by exactly one per Skip-routed file.

#### Scenario: File with only Warn matches is omitted from mirror under Skip

- **WHEN** the raw-mirror sync runs with on-hit policy `Skip` against a source file containing exactly one email match (Warn severity) and no Critical matches
- **THEN** the destination path under the raw mirror directory SHALL NOT exist after the sync completes AND the warn sink SHALL contain at least one line beginning with `pii warn: email`

#### Scenario: File with Critical matches is masked even under Skip policy

- **WHEN** the raw-mirror sync runs with on-hit policy `Skip` against a source file containing one AWS access key match (Critical severity)
- **THEN** the destination file SHALL exist with the AWS key substring replaced by `[REDACTED:aws-access-key]` AND the file SHALL NOT be skipped (Critical floor overrides Skip)

#### Scenario: Files without matches are mirrored normally under Skip

- **WHEN** the raw-mirror sync runs with on-hit policy `Skip` against a source file with zero matches
- **THEN** the destination path under the raw mirror directory SHALL exist with byte-identical content to the source

### Requirement: Mirror Mask Behavior

When the active on-hit policy is `Mask` (or when a file contains any `PiiSeverity::Critical` match regardless of on-hit policy), the system SHALL write a transformed copy of the content to the raw mirror destination in which each `matched_text` substring is replaced by the literal string `[REDACTED:<pattern_name>]`. Replacement SHALL preserve all non-matched bytes and SHALL be performed in descending byte-offset order so earlier replacements do not shift later match offsets. Under explicit `Mask` policy, both Critical and Warn matches SHALL be replaced. Under `Warn` or `Skip` policy where Critical matches force the file into Mask processing, ONLY the Critical matches SHALL be replaced — Warn matches in the same file SHALL pass through unchanged (they take their own policy path: warn-line-only or skip-as-individual-match-but-file-already-mirrored). The warn sink SHALL still receive one line per original match. The system SHALL increment a `pii_masked_matches` counter on the sync summary by exactly one per replaced match.

#### Scenario: Match substring replaced with redaction marker under explicit Mask

- **WHEN** the raw-mirror sync runs with on-hit policy `Mask` against a source file whose content is `pre AKIAIOSFODNN7EXAMPLE post`
- **THEN** the destination file content SHALL equal `pre [REDACTED:aws-access-key] post`

#### Scenario: Critical-only mask under Warn policy

- **WHEN** the raw-mirror sync runs with on-hit policy `Warn` against a source file containing both `AKIAIOSFODNN7EXAMPLE` (Critical) and `alice@example.com` (Warn)
- **THEN** the destination file SHALL contain `[REDACTED:aws-access-key]` substituted for the AWS key AND SHALL contain the literal `alice@example.com` unchanged AND the warn sink SHALL contain warning lines for both the Critical and the Warn match

##### Example: mixed-severity file under Warn policy

- **GIVEN** source content `auth setup: AKIAIOSFODNN7EXAMPLE — contact alice@example.com for help`
- **WHEN** sync runs with `on_hit: warn` (default)
- **THEN** destination content SHALL equal `auth setup: [REDACTED:aws-access-key] — contact alice@example.com for help` (Critical masked, Warn preserved)

#### Scenario: Multiple matches replaced in descending offset order under explicit Mask

- **WHEN** the raw-mirror sync runs with on-hit policy `Mask` against a source file whose content contains two matches at byte offsets 10 and 50
- **THEN** the destination file SHALL contain both replaced markers AND the byte content between offset 0 and the first replacement SHALL be byte-identical to the source

#### Scenario: Non-UTF-8 file falls through to verbatim copy under Mask

- **WHEN** the raw-mirror sync runs with on-hit policy `Mask` against a source file whose bytes are not valid UTF-8
- **THEN** the destination file SHALL be a byte-identical copy of the source AND no warn line SHALL be emitted (the regex scanner does not produce matches against non-UTF-8 input)
