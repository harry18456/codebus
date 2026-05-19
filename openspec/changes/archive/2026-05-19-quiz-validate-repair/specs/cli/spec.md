## MODIFIED Requirements

### Requirement: Subcommand Registration

The `codebus` binary SHALL register exactly eight subcommands at the top level: `init`, `goal`, `query`, `lint`, `fix`, `config`, `chat`, `quiz`. No other subcommand SHALL be registered. The `--help` and `--version` flags SHALL be available at both the binary level and per subcommand. The `config` subcommand SHALL itself expose three sub-actions (`set-key`, `get-key`, `delete-key`); the sub-action contract is defined normatively in the `claude-code-config` capability. The `quiz` subcommand SHALL itself expose a `validate` sub-action (`codebus quiz validate <quiz-md-file | -> [--json]`); the sub-action contract is defined normatively in the `Quiz Validate Sub-Action Behavior` requirement of this capability and the `Quiz Output Validation and Repair` requirement of the `quiz` capability. The `chat` subcommand contract is defined normatively in the `Chat Subcommand Behavior` requirement of this capability and the `Chat CLI Subcommand Behavior` requirement of the `chat-verb` capability. The `quiz` subcommand contract is defined normatively in the `Quiz Subcommand Behavior` requirement of this capability and the `quiz` capability.

#### Scenario: Help output lists exactly the eight subcommands

- **WHEN** `codebus --help` is invoked
- **THEN** the help output SHALL list `init`, `goal`, `query`, `lint`, `fix`, `config`, `chat`, `quiz` as the only subcommands AND SHALL exit with status zero

#### Scenario: Version flag prints cargo package version

- **WHEN** `codebus --version` is invoked
- **THEN** the output SHALL be the `codebus` cargo package version AND SHALL exit with status zero

#### Scenario: Quiz validate sub-action is registered under quiz

- **WHEN** `codebus quiz --help` is invoked
- **THEN** the help output SHALL document a `validate` sub-action AND the top-level `codebus --help` SHALL still list exactly the eight subcommands with no ninth top-level subcommand

## ADDED Requirements

### Requirement: Quiz Validate Sub-Action Behavior

The `codebus quiz validate <quiz-md-file | ->` sub-action SHALL run the deterministic quiz validator (defined normatively in the `quiz` capability's `Quiz Output Validation and Repair` requirement) over a quiz markdown body and SHALL share the same underlying validator function the library final-verify uses. The body source SHALL be either a file path argument OR standard input when the argument is `-` or omitted. Human output SHALL report zero issues and exit with status zero when the body has no schema or wikilink-existence findings, SHALL list each finding (question identifier, rule, message) and exit with status one when findings exist, and SHALL exit with status two on a setup error (no locatable vault, or an unreadable file argument). With `--json`, the sub-action SHALL emit a machine-readable findings array (each entry carrying at least `rule`, `severity`, the question identifier, and a message) and SHALL apply the same exit-code contract.

The stdin source exists so the codebus-quiz generate agent can self-validate the draft held in its context without first writing it to disk. This avoids a scratch-file lifecycle plus a write-then-emit double-write for a verb whose deliverable is a stdout body persisted by the caller; it is a process-simplicity choice and is NOT motivated by sandbox least-privilege (the `goal` and `fix` agents already run with un-gated vault Write).

The codebus-quiz agent's generate spawn sandbox SHALL grant the agent a Bash tool hard-gated to invoking only `codebus quiz validate` (whitelist specifier of the form `Bash(codebus quiz validate *)`), installed via the same PreToolUse hook mechanism the `lint-feedback-loop` capability defines for the codebus-fix agent. The generate toolset SHALL NOT add Write or Edit (the agent has no scratch file to write — it pipes its in-context draft via stdin). The always-blocked tool set (WebFetch, WebSearch, Task, MCP, and the other globally forbidden tools) SHALL remain blocked.

#### Scenario: Clean file exits zero

- **WHEN** `codebus quiz validate <file>` runs over a structurally valid quiz whose citations all resolve
- **THEN** human output SHALL report zero issues AND the process SHALL exit with status zero

#### Scenario: Findings exit one with details

- **WHEN** `codebus quiz validate <file>` runs over a quiz with a malformed question or a broken `[[slug]]` citation
- **THEN** the output SHALL list each finding with its question identifier and rule AND the process SHALL exit with status one

#### Scenario: JSON output is machine-readable

- **WHEN** `codebus quiz validate <file> --json` runs over a quiz with findings
- **THEN** the output SHALL be a JSON findings array where each entry carries at least `rule`, `severity`, the question identifier, and a message AND the process SHALL exit with status one

#### Scenario: Body is read from stdin when the argument is `-`

- **WHEN** a quiz markdown body is piped to `codebus quiz validate -`
- **THEN** the validator SHALL run over the piped body AND SHALL apply the same finding output and exit-code contract as the file-path form

#### Scenario: Agent Bash is hard-gated to quiz validate only

- **WHEN** the codebus-quiz generate spawn agent attempts a Bash command other than `codebus quiz validate ...`
- **THEN** the PreToolUse hook SHALL block it AND only `codebus quiz validate ...` invocations SHALL be permitted
