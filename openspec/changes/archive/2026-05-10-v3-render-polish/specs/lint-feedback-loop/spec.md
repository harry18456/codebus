## MODIFIED Requirements

### Requirement: Lint Output Formats

The lint subcommand SHALL support two output formats selectable via `--format <text|json>` (default: `text`).

The `text` format SHALL print a human-readable report grouped by file path, with each issue path expressed as a vault-relative path (e.g., `concepts/auth.md`, `index.md`). The text format SHALL include a coverage summary line stating the number of pages scanned, the number of nav files scanned, the error count, and the warning count. The text format SHALL be styled per the `Environment-Aware Output Styling` requirement of the `cli` capability: when `use_emoji` is true, the clean-result header SHALL begin with `✅`, the issue-result header SHALL begin with `🔍`, the per-file path lead SHALL be `✗ ` for files containing errors and `⚠ ` for files containing warnings only; when `use_emoji` is false, those leads SHALL be `ok`, `#`, `x ` and `! ` respectively (the legacy ASCII forms). When `use_color` is true, the issue severity tags SHALL be wrapped in ANSI color escapes — `error:` in red (`\x1b[31m...\x1b[0m`) and `warn: ` in yellow (`\x1b[33m...\x1b[0m`); the surrounding message text and rule identifier SHALL NOT be colored. When `use_hyperlinks` is true AND a non-empty Obsidian vault id is available for the vault root, each per-file path lead's `wiki/<rel-path>` substring SHALL be wrapped in an OSC 8 hyperlink whose target URL is `obsidian://open?vault=<percent-encoded-vault-id>&file=<percent-encoded-rel-path-from-wiki-root>`; when any of those preconditions is false, the path SHALL appear as plain text without OSC 8 escapes.

The `json` format SHALL print a single JSON object on stdout containing fields `vault_root` (absolute path string), `pages_scanned` (integer), `nav_files_scanned` (integer), `error_count` (integer), `warn_count` (integer), and `issues` (array). Each element of `issues` SHALL contain `path` (absolute filesystem path string, NOT vault-relative), `severity` (`"error"` or `"warn"`), `rule` (a stable kebab-case rule identifier from the rule set), and `message` (human-readable description). The JSON format SHALL NOT contain any emoji glyph, ANSI escape sequence, or OSC 8 hyperlink escape regardless of terminal capability.

The two formats SHALL never be mixed in the same invocation; selecting `--format json` SHALL suppress all human-readable text output to stdout.

#### Scenario: Text format emits vault-relative paths

- **WHEN** `codebus lint` runs without `--format json` against a vault with an issue in `wiki/concepts/auth.md`
- **THEN** stdout SHALL contain the substring `concepts/auth.md` AND SHALL NOT contain the substring of the absolute path leading up to that file

#### Scenario: Text format emoji header on clean vault when emoji enabled

- **WHEN** `codebus lint` runs against a vault with zero issues, in a TTY environment with `NO_COLOR` unset
- **THEN** stdout SHALL contain a line beginning with `✅` followed by the coverage substring `<N> page` where `<N>` is an integer

#### Scenario: Text format ASCII fallback when emoji disabled

- **WHEN** `codebus lint` runs against a vault with zero issues, with stdout redirected to a file (non-TTY)
- **THEN** stdout SHALL contain a line beginning with the literal token `ok` followed by the coverage substring AND SHALL NOT contain any emoji glyph

#### Scenario: Text format ANSI red error tag when color enabled

- **WHEN** `codebus lint` runs against a vault with one error in `wiki/concepts/foo.md`, in a TTY environment with `NO_COLOR` unset
- **THEN** stdout SHALL contain the substring `\x1b[31merror:\x1b[0m` (the ANSI red wrap on the `error:` tag)

#### Scenario: Text format ANSI yellow warn tag when color enabled

- **WHEN** `codebus lint` runs against a vault with one warning in `wiki/concepts/foo.md`, in a TTY environment with `NO_COLOR` unset
- **THEN** stdout SHALL contain the substring `\x1b[33mwarn: \x1b[0m` (the ANSI yellow wrap on the `warn: ` tag)

#### Scenario: Text format suppresses ANSI when NO_COLOR set

- **WHEN** `codebus lint` runs against a vault with one error, with `NO_COLOR=1` in the environment
- **THEN** stdout SHALL NOT contain any byte sequence beginning with `\x1b[` (no ANSI escapes at all)

#### Scenario: Text format wraps wiki path in OSC 8 hyperlink when vault id present

- **WHEN** `codebus lint` runs against a vault with one issue in `wiki/concepts/foo.md`, in a TTY environment that supports OSC 8, where `lookup_vault_id` returns `Some("abcdef")`
- **THEN** stdout SHALL contain the substring `\x1b]8;;obsidian://open?vault=abcdef&file=concepts/foo.md\x1b\\wiki/concepts/foo.md\x1b]8;;\x1b\\` (an OSC 8 wrap whose visible label is `wiki/concepts/foo.md` and whose URL targets the Obsidian vault opener)

#### Scenario: Text format omits OSC 8 when vault id absent

- **WHEN** `codebus lint` runs against a vault with one issue, in a TTY environment that supports OSC 8, where `lookup_vault_id` returns `None` (Obsidian config not present or vault not registered)
- **THEN** stdout SHALL contain the literal string `wiki/concepts/foo.md` AND SHALL NOT contain any OSC 8 escape sequence beginning with `\x1b]8;`

#### Scenario: Text format URL-encodes vault id and file path

- **WHEN** the resolved Obsidian vault id is `my vault` (containing a space) and the issue path is `processes/auth flow.md`
- **THEN** stdout SHALL contain the substring `vault=my%20vault&file=processes/auth%20flow.md` (with spaces percent-encoded as `%20`)

#### Scenario: JSON format emits absolute paths

- **WHEN** `codebus lint --format json` runs against a vault rooted at `<abs-vault>/wiki/` with an issue in `<abs-vault>/wiki/concepts/auth.md`
- **THEN** stdout SHALL contain a single JSON object whose `issues[].path` for that issue equals `<abs-vault>/wiki/concepts/auth.md` (absolute path)

#### Scenario: JSON format includes vault_root field

- **WHEN** `codebus lint --format json` runs against a vault rooted at `<abs-vault>/`
- **THEN** the JSON object SHALL contain a `vault_root` string field equal to the absolute path of `<abs-vault>/`

#### Scenario: JSON format suppresses human text and styling escapes

- **WHEN** `codebus lint --format json` runs against any vault, regardless of TTY / `NO_COLOR` / OSC 8 support
- **THEN** stdout SHALL parse as a single valid JSON value AND SHALL NOT contain emoji, ANSI color codes, OSC 8 hyperlink escapes, or any non-JSON prefix or suffix
