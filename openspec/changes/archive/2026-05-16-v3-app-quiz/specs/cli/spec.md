## MODIFIED Requirements

### Requirement: Subcommand Registration

The `codebus` binary SHALL register exactly eight subcommands at the top level: `init`, `goal`, `query`, `lint`, `fix`, `config`, `chat`, `quiz`. No other subcommand SHALL be registered. The `--help` and `--version` flags SHALL be available at both the binary level and per subcommand. The `config` subcommand SHALL itself expose three sub-actions (`set-key`, `get-key`, `delete-key`); the sub-action contract is defined normatively in the `claude-code-config` capability. The `chat` subcommand contract is defined normatively in the `Chat Subcommand Behavior` requirement of this capability and the `Chat CLI Subcommand Behavior` requirement of the `chat-verb` capability. The `quiz` subcommand contract is defined normatively in the `Quiz Subcommand Behavior` requirement of this capability and the `quiz` capability.

#### Scenario: Help output lists exactly the eight subcommands

- **WHEN** `codebus --help` is invoked
- **THEN** the help output SHALL list `init`, `goal`, `query`, `lint`, `fix`, `config`, `chat`, `quiz` as the only subcommands AND SHALL exit with status zero

#### Scenario: Version flag prints cargo package version

- **WHEN** `codebus --version` is invoked
- **THEN** the binary SHALL print a single line containing the cargo package version of the `codebus-cli` crate AND SHALL exit with status zero

#### Scenario: Unknown subcommand is rejected by clap

- **WHEN** `codebus mcp` or `codebus randomverb` is invoked
- **THEN** the binary SHALL print a clap error message to stderr identifying the unknown subcommand AND SHALL exit with non-zero status

#### Scenario: Config subcommand help lists its three actions

- **WHEN** `codebus config --help` is invoked
- **THEN** the help output SHALL list `set-key`, `get-key`, `delete-key` as the only sub-actions AND SHALL exit with status zero

#### Scenario: Chat subcommand help describes REPL behavior

- **WHEN** `codebus chat --help` is invoked
- **THEN** the help output SHALL describe the subcommand as launching an interactive multi-turn read-only chat REPL AND SHALL exit with status zero

#### Scenario: Quiz subcommand help describes quiz generation

- **WHEN** `codebus quiz --help` is invoked
- **THEN** the help output SHALL describe the subcommand as generating a read-only multiple-choice quiz from wiki pages AND SHALL document the `--count` flag AND SHALL exit with status zero

## ADDED Requirements

### Requirement: Quiz Subcommand Behavior

`codebus quiz "<topic>"` SHALL invoke `codebus_core::verb::quiz::run_quiz` with `QuizScope::Goal { text: <topic> }`. The subcommand SHALL accept an optional `--count <N>` flag (integer 3–10). When `--count` is omitted, the subcommand SHALL resolve `question_count` from the shared `quiz.default_length` config key, defaulting to 5 when that key is absent. The subcommand SHALL pass the agent the sandbox flags `--tools Read,Glob,Grep --allowedTools Read,Glob,Grep --permission-mode acceptEdits`. The subcommand SHALL be read-only and SHALL NOT auto-commit. The CLI SHALL NOT present an interactive scope-confirmation gate (the confirm gate is a GUI-only affordance); after the plan spawn emits scope, the CLI SHALL print the planned page list and proceed directly to the generate spawn. The CLI SHALL persist the resulting quiz file with caller-injected frontmatter per the `quiz` capability.

Exit status SHALL be zero on a successful quiz generation. A `[CODEBUS_QUIZ_NO_MATCH]` outcome SHALL be treated as a successful run (exit zero) with the no-match reason printed to stdout and no quiz file written. A spawn failure or `VerbError` SHALL produce a non-zero exit status.

#### Scenario: Quiz with explicit count

- **WHEN** `codebus quiz "JWT lifecycle" --count 7` is invoked against a vault whose wiki covers JWT
- **THEN** `run_quiz` SHALL be called with `question_count = 7` AND a quiz file with seven question sections SHALL be persisted AND the process SHALL exit zero

#### Scenario: Quiz count falls back to config then default

- **WHEN** `codebus quiz "auth"` is invoked with no `--count` AND `quiz.default_length` is absent from config
- **THEN** `run_quiz` SHALL be called with `question_count = 5`

#### Scenario: No-match exits zero without a file

- **WHEN** `codebus quiz "quantum mechanics"` is invoked against a vault whose wiki does not cover the topic
- **THEN** the CLI SHALL print the `[CODEBUS_QUIZ_NO_MATCH]` reason to stdout AND SHALL NOT write any quiz file AND SHALL exit with status zero

#### Scenario: Quiz does not auto-commit

- **WHEN** `codebus quiz "<topic>"` completes successfully in a git working tree
- **THEN** the subcommand SHALL NOT create any git commit
