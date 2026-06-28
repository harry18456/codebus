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

The system SHALL provide a default regex-based scanner that detects the following built-in PII categories, each with a stable `pattern_name` and a `severity`. The system SHALL compile all built-in patterns once at scanner construction; user-supplied extra patterns SHALL be rejected at construction if any fails to compile.

- `aws-access-key` (Critical): a token starting with `AKIA` or `ASIA` followed by exactly 16 uppercase alphanumerics.
- `anthropic-api-key` (Critical): a token starting with `sk-ant-` followed by 20 or more URL-safe characters.
- `github-pat` (Critical): a classic GitHub personal access token starting with one of `ghp_`, `gho_`, `ghu_`, `ghs_`, or `ghr_` followed by 36 base62 characters.
- `github-fine-grained-pat` (Critical): a fine-grained GitHub token starting with `github_pat_` followed by 82 characters from the set `[0-9A-Za-z_]`.
- `slack-token` (Critical): a Slack token starting with one of `xoxb-`, `xoxa-`, `xoxp-`, `xoxr-`, or `xoxs-` followed by hyphen-separated alphanumeric segments.
- `google-api-key` (Critical): a token starting with `AIza` followed by 35 characters from the set `[0-9A-Za-z_\-]`.
- `openai-api-key` (Critical): a token starting with `sk-proj-` followed by 20 or more URL-safe characters, OR starting with `sk-` followed by 20 or more characters from the set `[0-9A-Za-z]` (no hyphen or underscore in this second form, so an Anthropic `sk-ant-` token SHALL NOT match this pattern).
- `stripe-secret-key` (Critical): a token starting with `sk_live_` followed by 24 or more base62 characters.
- `pem-private-key` (Critical): a PEM private-key header of the form `-----BEGIN (RSA |EC |OPENSSH |DSA |PGP )?PRIVATE KEY-----`.
- `jwt` (Warn): a JSON Web Token shaped as three base64url segments separated by dots where the first two segments begin with `eyJ` (`eyJ...\.eyJ...\.<base64url>`).
- `db-connection-string` (Critical): a database URI whose scheme is one of `postgres`, `postgresql`, `mysql`, `mongodb`, `mongodb+srv`, `redis`, or `amqp`, followed by `://`, a userinfo component, a `:`, a password component, and an `@` (i.e. credentials are embedded in the URI).
- `email` (Warn): an RFC 5322-ish ASCII local part, an `@`, a domain containing at least one dot, and a TLD of two or more letters.
- `ipv4` (Warn): four dot-separated digit groups.

#### Scenario: Detects AWS access key

- **WHEN** the scanner is given content containing the substring `AKIAIOSFODNN7EXAMPLE`
- **THEN** the resulting match list SHALL contain exactly one entry whose `pattern_name` equals `aws-access-key` and whose `matched_text` equals `AKIAIOSFODNN7EXAMPLE`

#### Scenario: Detects Anthropic API key

- **WHEN** the scanner is given content containing `sk-ant-api01-abcDEF123456789_-XYZ012345`
- **THEN** the resulting match list SHALL contain exactly one entry whose `pattern_name` equals `anthropic-api-key`

#### Scenario: Detects classic GitHub personal access token

- **WHEN** the scanner is given content containing `ghp_` followed by 36 base62 characters
- **THEN** the resulting match list SHALL contain exactly one entry whose `pattern_name` equals `github-pat` and whose `severity` equals `Critical`

#### Scenario: Detects fine-grained GitHub token

- **WHEN** the scanner is given content containing `github_pat_` followed by 82 characters from `[0-9A-Za-z_]`
- **THEN** the resulting match list SHALL contain exactly one entry whose `pattern_name` equals `github-fine-grained-pat` and whose `severity` equals `Critical`

#### Scenario: Detects Slack token

- **WHEN** the scanner is given content containing a `xoxb-` prefix followed by hyphen-separated alphanumeric segments (at least 16 characters after the prefix)
- **THEN** the resulting match list SHALL contain exactly one entry whose `pattern_name` equals `slack-token` and whose `severity` equals `Critical`

#### Scenario: Detects Google API key

- **WHEN** the scanner is given content containing `AIza` followed by 35 characters from `[0-9A-Za-z_\-]`
- **THEN** the resulting match list SHALL contain exactly one entry whose `pattern_name` equals `google-api-key` and whose `severity` equals `Critical`

#### Scenario: Detects OpenAI API key

- **WHEN** the scanner is given content containing `sk-` followed by 48 characters from `[0-9A-Za-z]`
- **THEN** the resulting match list SHALL contain exactly one entry whose `pattern_name` equals `openai-api-key` and whose `severity` equals `Critical`

#### Scenario: OpenAI pattern does not match an Anthropic key

- **WHEN** the scanner is given content containing `sk-ant-api01-abcDEF123456789_-XYZ012345` and no other secret-shaped tokens
- **THEN** the resulting match list SHALL contain exactly one entry whose `pattern_name` equals `anthropic-api-key` AND SHALL NOT contain any entry whose `pattern_name` equals `openai-api-key`

#### Scenario: Detects Stripe secret key

- **WHEN** the scanner is given content containing `sk_live_` followed by 24 base62 characters
- **THEN** the resulting match list SHALL contain exactly one entry whose `pattern_name` equals `stripe-secret-key` and whose `severity` equals `Critical`

#### Scenario: Detects PEM private-key header

- **WHEN** the scanner is given content containing the line `-----BEGIN OPENSSH PRIVATE KEY-----`
- **THEN** the resulting match list SHALL contain exactly one entry whose `pattern_name` equals `pem-private-key` and whose `severity` equals `Critical`

#### Scenario: Detects JWT

- **WHEN** the scanner is given content containing `eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U`
- **THEN** the resulting match list SHALL contain exactly one entry whose `pattern_name` equals `jwt` and whose `severity` equals `Warn`

#### Scenario: Detects database connection string with embedded password

- **WHEN** the scanner is given content containing `postgres://dbuser:s3cr3tPassw0rd@db.internal:5432/app`
- **THEN** the resulting match list SHALL contain at least one entry whose `pattern_name` equals `db-connection-string` and whose `severity` equals `Critical`

#### Scenario: Detects email address

- **WHEN** the scanner is given content containing `alice@example.com`
- **THEN** the resulting match list SHALL contain exactly one entry whose `pattern_name` equals `email` and whose `matched_text` equals `alice@example.com`

#### Scenario: Detects IPv4 address

- **WHEN** the scanner is given content containing `192.168.1.42`
- **THEN** the resulting match list SHALL contain exactly one entry whose `pattern_name` equals `ipv4` and whose `matched_text` equals `192.168.1.42`

#### Scenario: Ignores AWS lookalike with insufficient trailing length

- **WHEN** the scanner is given content containing `AKIA12345ABCDEFGH` (15 trailing alphanumerics, not 16)
- **THEN** the resulting match list SHALL NOT contain any entry with `pattern_name` equal to `aws-access-key`

#### Scenario: Ignores GitHub-prefix lookalike with insufficient length

- **WHEN** the scanner is given content containing `ghp_short` (fewer than 36 trailing characters)
- **THEN** the resulting match list SHALL NOT contain any entry with `pattern_name` equal to `github-pat`

#### Scenario: Ignores database URI without an embedded password

- **WHEN** the scanner is given content containing `postgres://db.internal:5432/app` (host only, no `user:password@` userinfo)
- **THEN** the resulting match list SHALL NOT contain any entry with `pattern_name` equal to `db-connection-string`

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
source: v3-pii, pii-mirror-completeness
updated: 2026-06-26
code:
  - codebus-core/src/pii/scanners/regex_basic.rs
  - codebus-core/src/pii/mod.rs
  - codebus-core/src/pii/scanners/mod.rs
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


<!-- @trace
source: v3-pii-severity-dispatch
updated: 2026-05-10
code:
  - codebus-core/src/config/global_starter.rs
  - codebus-core/src/config/pii.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/vault/raw_sync.rs
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

The system SHALL select the active PII scanner implementation based on the `pii.scanner` config field. When the value is `regex_basic`, the system SHALL construct a `RegexBasicScanner` whose pattern set is the full built-in pattern set plus any entries from `pii.patterns_extra`. When the value is `none`, the system SHALL construct a `NullScanner` and SHALL NOT compile any regex from `pii.patterns_extra`.

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
source: v3-config, pii-mirror-completeness
updated: 2026-06-26
code:
  - codebus-core/src/pii/scanners/regex_basic.rs
  - codebus-core/src/pii/scanners/mod.rs
-->

---
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


<!-- @trace
source: v3-pii-severity-dispatch
updated: 2026-05-10
code:
  - codebus-core/src/config/global_starter.rs
  - codebus-core/src/config/pii.rs
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/vault/raw_sync.rs
-->

---
### Requirement: Mirror Mask Behavior

When the active on-hit policy is `Mask` (or when a file contains any `PiiSeverity::Critical` match regardless of on-hit policy), the system SHALL write a transformed copy of the content to the raw mirror destination in which each `matched_text` substring is replaced by the literal string `[REDACTED:<pattern_name>]`. Replacement SHALL preserve all non-matched bytes and SHALL be performed in descending byte-offset order so earlier replacements do not shift later match offsets. Overlapping or nested match spans (most plausibly when a custom `patterns_extra` regex frames a region containing an embedded built-in hit) SHALL be merged into the union span before substitution so the descending-replace strategy stays correct and no inner secret can survive between two outer substitutions; the merged span uses the earliest-starting contributing match's `pattern_name` as its `[REDACTED:...]` label, and adjacent (touching but non-overlapping) spans SHALL remain separate. Under explicit `Mask` policy, both Critical and Warn matches SHALL be replaced. Under `Warn` or `Skip` policy where Critical matches force the file into Mask processing, ONLY the Critical matches SHALL be replaced — Warn matches in the same file SHALL pass through unchanged (they take their own policy path: warn-line-only or skip-as-individual-match-but-file-already-mirrored). The warn sink SHALL still receive one line per original match. The system SHALL increment a `pii_masked_matches` counter on the sync summary by exactly one per replaced match. Content that is not valid UTF-8 but carries a UTF-16 LE, UTF-16 BE, or UTF-8 byte-order mark SHALL be decoded to UTF-8 before scanning so that secrets in BOM-marked UTF-16 text files are detected and masked; content that cannot be decoded by any of these means SHALL fall through to verbatim copy (see the `vault` capability's Raw Mirror with PII Scanner requirement for the unscanned-file counter).

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

#### Scenario: Overlapping matches merge into a single redaction span

- **WHEN** scanning produces two matches whose byte spans overlap or where one fully contains the other (e.g. a `patterns_extra` custom regex framing an embedded built-in email match) and the file is processed under Mask
- **THEN** the destination file SHALL contain a single `[REDACTED:<pattern_name>]` token covering the union of the two spans (labelled with the earliest-starting match's `pattern_name`) AND the inner match's `matched_text` SHALL NOT appear anywhere in the destination content

#### Scenario: Adjacent non-overlapping matches keep their own labels

- **WHEN** scanning produces two matches whose byte spans touch but do not overlap (i.e. one's `end` equals the other's `start`)
- **THEN** the destination file SHALL contain two separate `[REDACTED:<pattern_name>]` tokens — one per matching rule — placed back-to-back with no intervening source bytes

#### Scenario: BOM-marked UTF-16 file with a secret is decoded and masked

- **WHEN** the raw-mirror sync runs with on-hit policy `Mask` against a source file whose bytes are a UTF-16 LE byte-order mark followed by UTF-16 LE encoded text containing an AWS access key shape
- **THEN** the destination file SHALL contain `[REDACTED:aws-access-key]` substituted for the AWS key (the UTF-16 content having been decoded to UTF-8 before scanning) AND the original AWS key characters SHALL NOT appear in the destination content

#### Scenario: Undecodable non-UTF-8 binary file falls through to verbatim copy under Mask

- **WHEN** the raw-mirror sync runs with on-hit policy `Mask` against a source file whose bytes are not valid UTF-8 and carry no UTF-16 or UTF-8 byte-order mark (a true binary file)
- **THEN** the destination file SHALL be a byte-identical copy of the source AND no warn line SHALL be emitted (the regex scanner does not produce matches against undecodable input)

<!-- @trace
source: v3-pii-severity-dispatch, pii-mirror-completeness
updated: 2026-06-26
code:
  - codebus-core/src/vault/raw_sync.rs
-->

---
### Requirement: Warn Sink Location Format

When the raw-mirror sync emits a warn line for a PII match, the rendered location SHALL be a 1-based `line:col` position derived from the match start byte offset, formatted as `pii warn: <pattern_name> at <relative_path>:<line>:<col>`. The system SHALL NOT render the raw byte offset in the warn line. The line number SHALL be the 1-based count of `\n`-terminated lines preceding the match start plus one, and the column SHALL be the 1-based count of Unicode scalar values between the start of that line and the match start plus one. The match's `start`/`end` byte offsets are retained internally for masking and SHALL NOT change.

#### Scenario: Match on the first line renders 1-based line and column

- **WHEN** the raw-mirror sync emits a warn line for a match whose start byte offset falls on the first line of the source content (no preceding `\n`)
- **THEN** the warn line SHALL have the form `pii warn: <pattern_name> at <relative_path>:1:<col>` where `<col>` is the 1-based Unicode-scalar column of the match start

##### Example: email at column 9 on line 1

- **GIVEN** source file `docs.md` with content `contact alice@example.com\n`
- **WHEN** the email match (start at byte offset 8) is emitted under policy `Warn`
- **THEN** the warn line SHALL equal `pii warn: email at docs.md:1:9`

#### Scenario: Match on a later line renders the correct 1-based line number

- **WHEN** the raw-mirror sync emits a warn line for a match preceded by N newline characters in the source content
- **THEN** the rendered line number SHALL equal N + 1 AND the rendered column SHALL be relative to the start of that line, not the start of the file

##### Example: email on line 3

- **GIVEN** source file `logs.txt` with content `a\nb\ncontact alice@example.com\n` (two newlines before the match line)
- **WHEN** the email match is emitted under policy `Warn`
- **THEN** the warn line SHALL equal `pii warn: email at logs.txt:3:9`

#### Scenario: No raw byte offset appears in the warn line

- **WHEN** any warn line is emitted for a source file whose match start byte offset is greater than its total line count
- **THEN** the integer following the final colon SHALL be a valid 1-based column within its line AND the warn line SHALL NOT contain the raw byte offset value

<!-- @trace
source: pii-warn-location-line-col
updated: 2026-06-09
code:
  - codebus-core/src/vault/raw_sync.rs
-->

---
### Requirement: Empty and Zero-Width Extra Pattern Safety

The regex-based scanner SHALL be immune to empty and zero-width user-supplied extra patterns, which would otherwise produce a match at every character position and make scanning a large file pathologically slow.

At construction, the scanner SHALL skip any `patterns_extra` entry that is empty or whitespace-only after trimming: such an entry SHALL NOT be compiled and SHALL NOT become a rule. Skipping an empty entry SHALL NOT be treated as a compile failure (it is distinct from a malformed regex, which SHALL still fail fast at construction). Custom-pattern labels SHALL be assigned only to the retained non-empty entries, numbered contiguously starting at `custom-0`.

During scanning, the scanner SHALL NOT emit a match whose start offset equals its end offset (a zero-width match). This guard SHALL apply to every rule, so any pattern capable of matching zero-width input — an empty string, `a*`, `\b`, or `.*` against an empty region — cannot produce a per-character flood of matches.

#### Scenario: Empty extra pattern produces no rule and no matches

- **WHEN** the scanner is constructed with `patterns_extra` containing a single empty string AND is given non-empty content
- **THEN** construction SHALL succeed AND the empty entry SHALL NOT become a rule AND scanning SHALL return zero matches attributable to that entry

#### Scenario: Zero-width-capable pattern does not flood matches

- **WHEN** the scanner is constructed with an extra pattern that can match zero-width input such as `a*` AND is given content that does not contain a non-empty run of that pattern
- **THEN** scanning SHALL return zero matches for that pattern rather than one match per character position

##### Example: empty pattern on large content

- **GIVEN** `patterns_extra` is `[""]` AND content is a 250000-character string containing no secret-shaped tokens
- **WHEN** the scanner scans the content
- **THEN** the returned match list SHALL be empty, not one match per character position

#### Scenario: Non-empty custom pattern keeps contiguous numbering

- **WHEN** the scanner is constructed with `patterns_extra` equal to `["", "\\bINTERNAL-\\d{6}\\b"]` AND scans content containing `INTERNAL-123456`
- **THEN** the empty entry SHALL be skipped AND the single resulting custom match SHALL carry `pattern_name` equal to `custom-0`

<!-- @trace
source: config-save-robustness
updated: 2026-06-28
code:
  - codebus-core/src/pii/scanners/regex_basic.rs
tests:
  - codebus-core/src/pii/scanners/regex_basic.rs
-->
