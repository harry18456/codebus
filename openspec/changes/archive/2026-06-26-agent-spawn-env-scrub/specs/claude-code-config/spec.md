## MODIFIED Requirements

### Requirement: Scoped Environment Injection At Spawn

The `agent::claude_cli::invoke` function SHALL spawn the `claude` child process with a scrubbed environment. Before injecting any provider environment variables, the spawn path SHALL call `Command::env_clear()` on the child command to drop the inherited parent environment, and SHALL then re-populate the child environment with ONLY a fixed cross-platform allowlist of system-essential variables (the "passthrough set") read from the parent process. The passthrough set SHALL be platform-aware (a universal group plus OS-specific groups for Windows and Unix), SHALL include `PATH`, and SHALL be the SAME allowlist used by the codex spawn path (a single shared definition; the concrete membership lives in the implementation and design, and is NOT enumerated scenario-by-scenario here). Any parent environment variable that is NOT a member of the passthrough set — including secret-bearing variables such as `GITHUB_TOKEN`, `AWS_*`, `KUBECONFIG`, and `CODEBUS_AZURE_KEY` — SHALL NOT be visible to the spawned child process.

All environment variables SHALL be set on the child exclusively via the `Command::env_clear` / `Command::env` / `Command::envs` Rust API. Codebus SHALL NOT modify the parent process's environment (no `std::env::set_var` calls) at any point in the spawn pipeline; `Command::env_clear` SHALL act only on the child command and SHALL NOT affect the parent process environment.

The injection order SHALL be: `env_clear`, then the allowlist passthrough, then the profile-specific provider injection. When the system profile is active, codebus SHALL inject zero additional provider environment variables (the child still receives the allowlist passthrough). When the azure profile is active, codebus SHALL inject exactly three provider environment variables on the child process on top of the allowlist passthrough: `ANTHROPIC_BASE_URL` (from `azure.base_url`), `ANTHROPIC_API_KEY` (from the keyring fallback chain), and `CLAUDE_CODE_DISABLE_ADVISOR_TOOL` set to the literal string `1`.

#### Scenario: System profile scrubs parent env to the allowlist

- **WHEN** the system profile is active, the parent environment contains a non-allowlist secret variable (for example `GITHUB_TOKEN`), and `codebus query` is invoked
- **THEN** the spawned `claude` child process environment SHALL NOT contain the non-allowlist secret variable, SHALL contain the passthrough member `PATH`, AND no provider environment variables SHALL be added by codebus

#### Scenario: Azure profile injects three env vars on top of the allowlist

- **WHEN** the azure profile is active with `base_url=https://example.cognitiveservices.azure.com/anthropic`, the keyring key resolves to `sk-test`, the parent environment contains a non-allowlist secret variable, and `codebus query` is invoked
- **THEN** the spawned `claude` child process environment SHALL contain `ANTHROPIC_BASE_URL=https://example.cognitiveservices.azure.com/anthropic`, `ANTHROPIC_API_KEY=sk-test`, AND `CLAUDE_CODE_DISABLE_ADVISOR_TOOL=1`, SHALL contain the passthrough member `PATH`, AND SHALL NOT contain the non-allowlist secret variable

#### Scenario: Parent shell env is not modified

- **WHEN** the azure profile is active and `codebus query` runs to completion
- **THEN** the parent process's `ANTHROPIC_API_KEY` environment variable SHALL be observable from the parent shell as either unset OR retaining its pre-invocation value (the `env_clear` scrub acts only on the child command, never on the parent process)
