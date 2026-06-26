## ADDED Requirements

### Requirement: Spawn Environment Scrub

The `CodexBackend::build_command` method SHALL spawn the `codex` child process with a scrubbed environment. After constructing the `Command` and before injecting the Azure API key, `build_command` SHALL call `Command::env_clear()` to drop the inherited parent environment, and SHALL then re-populate the child environment with ONLY the same cross-platform system-essential allowlist (the "passthrough set") used by the claude spawn path — a single shared definition (see the `claude-code-config` capability's `Scoped Environment Injection At Spawn` requirement). The passthrough set SHALL include `PATH`. Any parent environment variable that is NOT a member of the passthrough set — including secret-bearing variables such as `GITHUB_TOKEN`, `AWS_*`, and `KUBECONFIG` — SHALL NOT be visible to the spawned `codex` child process. `Command::env_clear` SHALL act only on the child command and SHALL NOT modify the parent process environment.

When the azure profile is active, codebus SHALL inject the Azure API key environment variable (`CODEBUS_CODEX_AZURE_KEY`) on the child AFTER the scrub, so the key reaches the codex provider override. The injection order SHALL be: `env_clear`, then the allowlist passthrough, then the Azure key injection.

#### Scenario: Codex spawn scrubs parent env to the allowlist

- **WHEN** `CodexBackend::build_command` builds the spawn command and the parent environment contains a non-allowlist secret variable (for example `GITHUB_TOKEN`)
- **THEN** the spawned `codex` child process environment SHALL NOT contain the non-allowlist secret variable AND SHALL contain the passthrough member `PATH`

#### Scenario: Codex azure key survives the scrub

- **WHEN** the azure profile is active, the Azure key resolves to `sk-codex-test`, and `CodexBackend::build_command` builds the spawn command
- **THEN** the spawned `codex` child process environment SHALL contain `CODEBUS_CODEX_AZURE_KEY=sk-codex-test` (injected after `env_clear`) AND SHALL contain the passthrough member `PATH`
