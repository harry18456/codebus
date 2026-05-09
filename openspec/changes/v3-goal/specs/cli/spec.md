## ADDED Requirements

### Requirement: Goal Subcommand Behavior

The `goal` subcommand SHALL accept one positional argument `<goal-text>` (the user's natural-language goal description) and the following flags: `--repo <PATH>` (default: current working directory), `--force-resync` (boolean, force raw mirror re-sync even when source-signal detection reports no drift), `--no-obsidian-register` (boolean, forwarded to the auto-init fallback), and the inherited `--debug` global flag. When invoked, the subcommand SHALL execute exactly five steps in order: (1) resolve the target repository path, (2) when `<repo>/.codebus/` does not exist, run the `init` flow against the same repo (forwarding `--no-obsidian-register`); (3) perform source-signal drift detection and conditionally re-sync the raw mirror plus update the manifest; (4) spawn the agent CLI with the canonical sandbox flags described below and stream child stdout and stderr to the parent process; (5) after the child terminates, regardless of whether the child exit code is zero or non-zero, invoke the vault auto-commit operation with the message `wiki: <goal-text>` against the nested vault repository. The subcommand SHALL exit with the child process's exit code unless the auto-commit operation itself fails, in which case it SHALL exit with a non-zero code identifying the auto-commit failure.

The agent spawn SHALL pass the following arguments to the agent CLI: `-p` followed by the literal string `/codebus-goal "<goal-text>"`; `--tools` followed by the comma-separated string `Read,Glob,Grep,Write,Edit`; `--allowedTools` followed by the same comma-separated string; `--permission-mode` followed by `acceptEdits`. The agent's child process SHALL be invoked with the current working directory set to `<repo>/.codebus/` and its stdin SHALL be a closed stream (preventing the agent from blocking on input).

#### Scenario: Goal subcommand invokes auto-init when vault is missing

- **WHEN** `codebus goal "describe the auth flow"` is invoked against a repository whose `.codebus/` directory does not yet exist
- **THEN** the subcommand SHALL first run the init flow (creating `.codebus/`, vault layout, raw mirror, schema, manifest, skill bundles, nested git, and the initial `init: codebus vault` commit) AND afterwards SHALL proceed to spawn the agent against the freshly-initialized vault

#### Scenario: Goal subcommand spawns agent with cwd at vault root

- **WHEN** the goal subcommand reaches its agent-spawn step against a vault rooted at `<repo>/.codebus/`
- **THEN** the agent child process SHALL be spawned with current working directory equal to `<repo>/.codebus/` AND the child SHALL receive the prompt argument `/codebus-goal "<goal-text>"` AND the child's stdin SHALL be closed

#### Scenario: Goal subcommand passes the canonical triple-flag sandbox

- **WHEN** the goal subcommand spawns the agent
- **THEN** the spawned command line SHALL include the flag pair `--tools Read,Glob,Grep,Write,Edit`, the flag pair `--allowedTools Read,Glob,Grep,Write,Edit`, and the flag pair `--permission-mode acceptEdits`

#### Scenario: Goal subcommand auto-commits on agent success

- **WHEN** the goal subcommand spawns the agent and the agent exits with status zero after writing one or more files inside `<repo>/.codebus/wiki/`
- **THEN** after the child terminates the subcommand SHALL invoke the vault auto-commit operation with message `wiki: <goal-text>` AND running `git -C <repo>/.codebus log --pretty=%s -1` SHALL print exactly the line `wiki: <goal-text>`

#### Scenario: Goal subcommand auto-commits on agent failure with partial writes

- **WHEN** the goal subcommand spawns the agent and the agent exits with non-zero status after writing one or more files inside `<repo>/.codebus/wiki/`
- **THEN** the subcommand SHALL still invoke the vault auto-commit operation with message `wiki: <goal-text>` AND running `git -C <repo>/.codebus log --pretty=%s -1` SHALL print exactly `wiki: <goal-text>` AND the subcommand SHALL propagate the agent's non-zero exit code

#### Scenario: Goal subcommand no-op commit when agent makes no changes

- **WHEN** the goal subcommand spawns the agent and the agent exits without modifying any file under `<repo>/.codebus/wiki/`
- **THEN** the auto-commit operation SHALL be a no-op (working tree clean) AND `git -C <repo>/.codebus rev-list --count HEAD` SHALL equal the same count as before the goal invocation

#### Scenario: Force-resync flag bypasses detection

- **WHEN** `codebus goal "..." --force-resync` is invoked against a vault whose source signal would otherwise be unchanged
- **THEN** the subcommand SHALL re-run the raw mirror unconditionally AND SHALL update the manifest's `last_sync_at` timestamp regardless of whether the source content changed

#### Scenario: Goal subcommand propagates agent exit code

- **WHEN** the goal subcommand spawns the agent and the agent terminates with exit status code N (where N is any non-negative integer)
- **THEN** the goal subcommand SHALL exit with status code N unless the post-spawn auto-commit operation itself fails
