## ADDED Requirements

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

---

### Requirement: Null Scanner Behavior

The system SHALL provide a no-op scanner whose `scan` method returns an empty match list for any input regardless of input content.

#### Scenario: Null scanner returns empty for clean input

- **WHEN** the null scanner scans the content `hello world`
- **THEN** the returned match list SHALL be empty

#### Scenario: Null scanner returns empty even when input contains secret-shaped tokens

- **WHEN** the null scanner scans content containing the substring `AKIAIOSFODNN7EXAMPLE`
- **THEN** the returned match list SHALL be empty

---

### Requirement: On-Hit Policy Default

The system SHALL define three on-hit policy values: `Warn`, `Skip`, and `Mask`. The default value SHALL be `Warn`. Selection of `Skip` or `Mask` SHALL be deferred to a future capability that exposes user configuration; in this change the system SHALL hardcode the default and SHALL NOT read configuration to override it.

#### Scenario: Default on-hit policy value is Warn

- **WHEN** an on-hit policy value is requested without an explicit override
- **THEN** the resulting value SHALL equal `Warn`

#### Scenario: No configuration file is read for on-hit selection

- **WHEN** the system selects the on-hit policy during raw mirror execution
- **THEN** the system SHALL NOT read `~/.codebus/config.yaml` or any other configuration file to determine the policy
