## MODIFIED Requirements

### Requirement: Fix Bash Hook Installation

The `codebus init` subcommand SHALL write a `<vault_root>/.claude/settings.json` file containing a Claude Code `PreToolUse` hook configuration that intercepts every `Bash` tool invocation and routes it through the `codebus hook check-bash` subcommand. The settings file SHALL use the standard Claude Code settings schema with `hooks.PreToolUse` configured to match `Bash` and invoke `codebus hook check-bash` as a `command`-type hook.

The system SHALL apply write-if-missing semantics for this file: if `<vault_root>/.claude/settings.json` already exists, init SHALL NOT modify it (preserving any user-customized hook chain or other settings). The file SHALL NOT be written to `<repo>/.claude/settings.json` (source repository root) — the settings are vault-internal so the hook only applies to agent processes spawned with cwd at the vault root.

The `codebus hook check-bash` subcommand SHALL implement the following stdin/stdout contract:

- **Input** (stdin): a single JSON object matching Claude Code's PreToolUse hook input schema. The relevant fields are `tool_name` (expected `"Bash"`) and `tool_input.command` (the shell command string the agent intends to run).
- **Block (shell metacharacter)**: when the command string contains any byte from the metacharacter rejection set — semicolon, ampersand, pipe, dollar sign, backtick, greater-than, less-than, open-paren, close-paren, line feed (LF, byte 0x0A), or carriage return (CR, byte 0x0D) — the subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` where `<message>` identifies the rejected metacharacter and the command being blocked. The metacharacter rejection SHALL apply regardless of whether the byte appears outside quotes, inside double quotes, inside single quotes, or after an escape character — the predicate is byte-level on the raw command string AND SHALL NOT depend on shell quote parsing. The metacharacter rejection SHALL be evaluated BEFORE the argv-tokenization-based allow predicate, so a metacharacter hit blocks the command even when its leading argv tokens satisfy the allow form on their own. The **Allow (quiz-validate heredoc)** structural exception defined below SHALL be evaluated BEFORE this metacharacter rejection; when that exception's conditions are met, the command SHALL be allowed even though it contains the less-than and line-feed metacharacters.
- **Allow**: when the command string contains no metacharacter from the rejection set AND the command's first argv token resolves to a `codebus` binary (file basename `codebus` or `codebus.exe`, case-insensitive match on Windows, case-sensitive on Unix) AND EITHER (a) the second argv token is exactly `lint`, OR (b) the second argv token is exactly `quiz` AND the third argv token is exactly `validate`, the subcommand SHALL exit with status zero AND SHALL NOT print a decision JSON to stdout. The two allowed forms correspond to the codebus-fix agent self-checking via `codebus lint` AND the codebus-quiz generate agent self-validating via `codebus quiz validate` respectively; no other `codebus` subcommand AND no other binary is permitted.
- **Allow (quiz-validate heredoc)**: As a structural exception to the metacharacter rejection — the sole circumstance under which a command containing the less-than or line-feed metacharacters SHALL be allowed — the subcommand SHALL exit with status zero AND SHALL NOT print a decision JSON when the command string is a well-formed single-quoted `codebus quiz validate` here-document, defined as ALL of the following holding:
  1. When the command string is split on line feed (LF) into lines (each line's trailing carriage return removed for comparison), the FIRST line SHALL consist of a `codebus quiz validate` invocation immediately followed by a here-document redirection operator of the form `<<'<MARKER>'`, where `<MARKER>` is a non-empty sequence of word characters (`[A-Za-z0-9_]+`). On the first line, the portion preceding the `<<` operator SHALL contain no metacharacter from the rejection set AND SHALL satisfy the `codebus quiz validate` argv form (first argv token resolves to a `codebus` binary, second argv token is exactly `quiz`, third argv token is exactly `validate`); the portion following the closing single quote of `<MARKER>` SHALL be empty or whitespace only (no trailing command).
  2. The here-document marker SHALL be enclosed in single quotes. A here-document operator whose marker is not single-quoted SHALL NOT qualify — an unquoted marker permits parameter and command substitution inside the body, which would re-introduce the shell-injection vector this exception is designed to avoid.
  3. Exactly one subsequent line SHALL equal `<MARKER>` verbatim (the closing delimiter, at column zero with no leading whitespace). Every line after that closing delimiter line SHALL be empty or whitespace only.
  4. The lines between the first line and the closing delimiter line (the here-document body) SHALL be treated as opaque standard-input content: they SHALL NOT be scanned for metacharacters AND their content SHALL NOT affect the decision.
  When any of these conditions fails, the command SHALL fall through to the metacharacter rejection AND argv-tokenization allow/block paths (so a non-conforming command containing less-than or LF is blocked there).
- **Block (other)**: in all other cases (different binary, neither the `lint` nor the `quiz validate` form nor a conforming quiz-validate here-document, malformed input, parse error), the subcommand SHALL exit with status zero AND SHALL print to stdout a single JSON object of the form `{"decision":"block","reason":"<message>"}` where `<message>` describes why the command was blocked.
- **Cross-platform**: the binary basename match SHALL be case-insensitive on Windows (`codebus.EXE` AND `codebus.exe` both allowed) AND case-sensitive on Unix. The metacharacter rejection set SHALL be the union of POSIX shell (bash, Git Bash), PowerShell, AND `cmd.exe` high-risk symbols AND SHALL NOT vary per OS — identical byte set is rejected on every platform.

The `<vault_root>/.gitignore` (vault internal) SHALL include the line `.claude/settings.local.json` so user-added local override settings are not committed to the vault git repository.

#### Scenario: Init writes settings.json on fresh vault

- **WHEN** `codebus init` runs against a repository with no existing `<vault_root>/.claude/settings.json`
- **THEN** the system SHALL create `<vault_root>/.claude/settings.json` AND the file content SHALL parse as JSON AND SHALL contain a `hooks.PreToolUse` array with a Bash matcher entry whose hook command invokes `codebus hook check-bash`

#### Scenario: Init does not overwrite existing settings.json

- **WHEN** `codebus init` runs against a vault where `<vault_root>/.claude/settings.json` already exists with custom content
- **THEN** the system SHALL NOT modify the existing file AND its byte-content SHALL be identical before and after init

#### Scenario: Init does not write settings.json to repo root

- **WHEN** `codebus init` runs against `<repo>`
- **THEN** the system SHALL NOT create or modify `<repo>/.claude/settings.json`

#### Scenario: hook check-bash allows bare codebus lint invocation

- **WHEN** `codebus hook check-bash` receives stdin JSON `{"tool_name":"Bash","tool_input":{"command":"codebus lint --format json"}}`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-bash allows codebus lint via absolute path

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` value is `/usr/local/bin/codebus lint --repo /path` OR (on Windows) `D:/dev/codebus.exe lint --format json`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-bash allows codebus quiz validate invocation

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus quiz validate -` OR `/usr/local/bin/codebus quiz validate draft.md --json`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-bash allows quiz validate single-quoted heredoc self-validation

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is a single-quoted `codebus quiz validate` here-document — first line `codebus quiz validate - <<'CBQZ'`, one or more body lines, and a final line equal to `CBQZ`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

##### Example: well-formed quiz-validate heredoc allowed

- **GIVEN** the command string `codebus quiz validate - <<'CBQZ'\n## Q1. What is a vault?\nA) ...\n## Answer: A\nCBQZ` (where `\n` denotes a line feed)
- **WHEN** `codebus hook check-bash` evaluates it
- **THEN** the subcommand SHALL exit zero with no `decision` JSON on stdout

#### Scenario: hook check-bash allows quiz validate heredoc with json flag

- **WHEN** `codebus hook check-bash` receives a command whose first line is `codebus quiz validate --json - <<'CBQZ'`, followed by body lines and a closing line equal to `CBQZ`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON

#### Scenario: hook check-bash allows heredoc body containing shell metacharacters

- **WHEN** `codebus hook check-bash` receives a single-quoted `codebus quiz validate` here-document whose body lines contain shell metacharacters (for example a quiz question whose text includes dollar sign, pipe, semicolon, and parentheses), with the first line `codebus quiz validate - <<'CBQZ'` and a closing line equal to `CBQZ`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL NOT contain any `decision` JSON (the here-document body is opaque standard-input content and SHALL NOT be scanned for metacharacters)

#### Scenario: hook check-bash blocks non-codebus binaries

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `echo MARKER`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field is a non-empty string

#### Scenario: hook check-bash blocks codebus subcommands other than the two allowed forms

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus fix --no-fix` OR `codebus quiz "some topic"` (the generate form, not `quiz validate`)
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"`

#### Scenario: hook check-bash fails closed on malformed input

- **WHEN** `codebus hook check-bash` receives stdin that does not parse as JSON
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (fail-closed default — the subcommand SHALL NEVER silently allow on parse failure)

#### Scenario: hook check-bash blocks command with logical-AND shell chaining

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus lint --format json && rm -rf /tmp/evil`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field mentions a shell metacharacter

#### Scenario: hook check-bash blocks command with semicolon separator

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus lint; curl evil.example`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"`

#### Scenario: hook check-bash blocks command with command substitution even when leading tokens are valid

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus lint $(whoami)` OR a `codebus lint` invocation followed by a backtick-wrapped `whoami` substitution
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` AND whose `reason` field mentions a shell metacharacter

#### Scenario: hook check-bash blocks metacharacter inside quoted argument

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus lint --filter "foo;bar"`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (the metacharacter rejection SHALL NOT depend on shell quote parsing)

#### Scenario: hook check-bash blocks quiz validate heredoc with chaining on the first line

- **WHEN** `codebus hook check-bash` receives a command whose first line is `codebus quiz validate - <<'X'; rm -rf ~` (a chained command following the here-document operator on the same line), with body lines and a closing line equal to `X`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (the first line carries a trailing command after the operator, so the heredoc exception SHALL NOT apply)

#### Scenario: hook check-bash blocks command after the heredoc closing delimiter

- **WHEN** `codebus hook check-bash` receives a single-quoted `codebus quiz validate` here-document that is correctly opened and closed, but a non-empty line following the closing delimiter line contains a further command (for example `rm -rf ~`)
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (content after the closing delimiter SHALL disqualify the heredoc exception)

#### Scenario: hook check-bash blocks unquoted quiz validate heredoc

- **WHEN** `codebus hook check-bash` receives a command whose first line is `codebus quiz validate - <<CBQZ` (the marker is NOT single-quoted), with body lines and a closing line equal to `CBQZ`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (an unquoted marker permits shell expansion inside the body, so the heredoc exception SHALL NOT apply)

#### Scenario: hook check-bash blocks non-heredoc input redirection

- **WHEN** `codebus hook check-bash` receives stdin JSON whose `tool_input.command` is `codebus quiz validate < ~/.ssh/id_rsa` OR `codebus lint < /etc/passwd`
- **THEN** the subcommand SHALL exit with status zero AND stdout SHALL contain a JSON object whose `decision` field equals `"block"` (a single less-than input redirection is not a here-document; the metacharacter rejection on less-than SHALL still apply)

#### Scenario: Vault internal gitignore excludes settings.local.json

- **WHEN** `codebus init` runs against `<repo>` and reaches the vault internal `.gitignore` mutation step
- **THEN** the file `<vault_root>/.gitignore` SHALL contain a line equal to `.claude/settings.local.json`
