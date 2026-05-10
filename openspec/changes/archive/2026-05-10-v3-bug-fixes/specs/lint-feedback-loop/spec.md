## MODIFIED Requirements

### Requirement: Vault Root Auto-Detection

The lint subsystem SHALL determine the vault root using the following precedence on each invocation:

1. If the `--repo <PATH>` flag is provided, the system SHALL inspect `<PATH>` to disambiguate the user's intent: when `<PATH>/wiki/` is a directory the system SHALL treat `<PATH>` itself as the vault root (the user passed the `.codebus/` directory directly); otherwise the system SHALL treat `<PATH>` as the source repository root and use `<PATH>/.codebus/` as the vault root.
2. Otherwise, if the current working directory contains a `wiki/` subdirectory, the system SHALL treat the current working directory itself as the vault root.
3. Otherwise, if the current working directory contains a `.codebus/wiki/` path, the system SHALL treat the current working directory as the source repository root and `<cwd>/.codebus/` as the vault root.
4. Otherwise, the system SHALL exit with status 2 and emit a stderr hint instructing the user to run `codebus init` first.

The auto-detection SHALL apply only to the lint subcommand and the lint phase of the fix subcommand. The `init`, `goal`, and `query` subcommands SHALL retain their existing behavior of treating the input path as the source repository root.

#### Scenario: Lint detects vault when cwd is vault root itself

- **WHEN** `codebus lint` is invoked from a working directory containing a `wiki/` subdirectory
- **THEN** the lint SHALL operate on `<cwd>/wiki/` AND SHALL NOT look for `<cwd>/.codebus/`

#### Scenario: Lint detects vault when cwd is source repo root

- **WHEN** `codebus lint` is invoked from a working directory containing `.codebus/wiki/` but not a direct `wiki/` subdirectory
- **THEN** the lint SHALL operate on `<cwd>/.codebus/wiki/`

#### Scenario: Lint refuses when no vault is locatable

- **WHEN** `codebus lint` is invoked from a directory containing neither `wiki/` nor `.codebus/wiki/`, with no `--repo` flag
- **THEN** the system SHALL exit with status 2 AND stderr SHALL contain a message instructing the user to run `codebus init`

#### Scenario: Explicit --repo flag pointing at source repo uses joined path

- **WHEN** `codebus lint --repo /path/to/source-repo` is invoked, where `/path/to/source-repo` does NOT contain a `wiki/` subdirectory but its `.codebus/wiki/` path exists
- **THEN** the system SHALL use `/path/to/source-repo/.codebus/` as the vault root

#### Scenario: Explicit --repo flag pointing at vault root uses path directly

- **WHEN** `codebus lint --repo /path/to/source-repo/.codebus` is invoked, where the path itself contains a `wiki/` subdirectory
- **THEN** the system SHALL use `/path/to/source-repo/.codebus/` as the vault root (no second `.codebus` join) AND lint SHALL produce identical output to invoking `codebus lint --repo /path/to/source-repo`

##### Example: same vault, two --repo argument forms

- **GIVEN** an initialized vault at `/repo/.codebus/wiki/` with one broken wikilink in `concepts/foo.md`
- **WHEN** the user runs `codebus lint --repo /repo` and separately `codebus lint --repo /repo/.codebus`
- **THEN** both invocations SHALL emit the same stdout report containing the broken-wikilink warning for `wiki/concepts/foo.md`

#### Scenario: Explicit --repo flag with non-existent path falls back to joined form

- **WHEN** `codebus lint --repo /nonexistent/path` is invoked, where neither `/nonexistent/path/wiki/` nor `/nonexistent/path/.codebus/wiki/` exists
- **THEN** the system SHALL use `/nonexistent/path/.codebus/` as the vault root (preserving the prior contract that `--repo` does not validate existence — the lint phase that follows will surface the absence of any wiki content)
