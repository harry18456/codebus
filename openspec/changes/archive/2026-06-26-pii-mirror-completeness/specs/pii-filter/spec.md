## MODIFIED Requirements

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
