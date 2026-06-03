# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

codebus is a CLI (`codebus`) + Tauri desktop app that drives an **agent CLI** (Claude Code or OpenAI Codex) to build and maintain an Obsidian-compatible wiki of an unfamiliar codebase, persisted under the target repo's `.codebus/` vault. The verbs the agent is driven through are `init` / `goal` / `query` / `fix` / `chat` / `quiz` / `lint`. Rust workspace, edition 2024, rust 1.85+.

## Commands

Rust (workspace = `codebus-core`, `codebus-cli`, `codebus-app/src-tauri`):

- Build a crate: `cargo build -p codebus-core` · whole workspace: `cargo build`
- Install the CLI on PATH: `cargo install --path codebus-cli` — **required for `fix`**, which shells out to `codebus lint` and blind-fixes if it isn't on PATH.
- Test: `cargo test -p codebus-core` (lib + integration). `cargo test -p codebus-cli` runs the CLI integration tests, which drive a **mock spawn binary** (`codebus-cli/tests/bins/mock_claude.rs`) rather than a real agent.
- Single test: `cargo test -p codebus-core <name-substring>` or `cargo test -p codebus-cli --test <file> <name>` (e.g. `--test quiz_flow`).
- Live-agent tests (real Claude/Codex, spend API) are `#[ignore]` + env-gated (e.g. `CODEBUS_LIVE_CODEX=1`, run with `-- --ignored`).
- Lint: `cargo clippy --workspace` — a small pre-existing baseline of warnings exists; the bar is **no new** warnings, not zero.

App (run inside `codebus-app/`):

- `npm run test` (Vitest) · `npm run typecheck` (`tsc --noEmit`) · `npm run build`
- Desktop app: `npm run tauri dev`

Env overrides (useful for tests / non-default setups): `CODEBUS_CLAUDE_BIN` / `CODEBUS_CODEX_BIN` (default `claude` / Windows `codex.cmd`), `CODEBUS_HOME` (relocates `~/.codebus`), `CODEBUS_CODEX_AZURE_KEY` (codex Azure profile key, injected via scoped child env, never argv).

## Architecture (the parts that span multiple files)

**Provider-neutral agent seam** — the central abstraction. A verb builds a `SpawnSpec` (provider-neutral intent: `verb` / `permission` / `sub_mode` / `input` / `resume_session_id`, NOT a pre-composed prompt). An `AgentBackend` trait (`agent/backend.rs`) has three methods — `build_command` / `parse_stream_line` / `extract_session_id` — implemented by `ClaudeBackend` and `CodexBackend`. The single loop in `agent/claude_cli.rs::invoke` is **provider-agnostic despite its filename**: it owns spawn / stdio piping / cancel polling / wall-clock timeout / token accumulation / process-tree kill, and delegates everything provider-specific to the backend. `agent/dispatch.rs` selects the backend from `agent.active_provider`. When touching `invoke`, do not leak a provider name into it — provider differences are carried on the backend or tagged on events (e.g. token cumulative-vs-delta via `AgentBackend::token_usage_semantics`).

**Verb layer** (`verb/`: goal/query/fix/chat/quiz) — each constructs a `SpawnSpec` + toolset/permission and calls `invoke`. The verb library **never reads config itself**; the caller (a `codebus-cli` command or a `codebus-app` Tauri IPC handler) resolves model / effort / timeout / content-verify gates from config and injects them. This keeps `codebus-core` caller-agnostic so CLI and app share it.

**Config** (`config/`) — `~/.codebus/config.yaml` `agent.providers.<claude|codex>` with `active: system|azure` profiles and per-verb `{model, effort}` sub-blocks (`goal/query/fix/verify`). The active profile must be fully populated; the non-active profile is cold-storage (preserved, not validated). claude `model` is a free-string alias (`opus-4-6` → `--model claude-opus-4-6`, forward-compatible); `effort` is the closed set `low/medium/high/xhigh/max` (no `auto` — the CLI rejects it).

**Skill / instruction materialization** (`skill_bundle/`) — the agent's per-verb `SKILL.md` prompt bodies (source constants like `FIX_WORKFLOW` in `skill_bundle/mod.rs`) are written into each vault's `.codebus/.claude/skills/` and `.codebus/.codex/skills/`. Codex bodies are **derived from the claude bodies** by `claude_to_codex_translate` (literal `str.replace` over `CODEX_BODY_TRANSLATIONS`), drift-guarded by tests so editing a claude SKILL can't silently leave the codex body stale/wrong. Edit the source constants — the materialized vault files are write-if-missing and `.gitignore`d.

**Stream + log** (`stream/`, `log/`) — provider JSONL → neutral `StreamEvent` (`codex_parser.rs` / `parser.rs`); `log/sink.rs` writes one `RunLog` row per verb invocation (mode/outcome/tokens/session_id/interrupt_reason/sandbox_denial_count) plus a per-run `events-*.jsonl`.

**Security model** — the agent reads a PII-redacted **mirror** (`pii/` scanner + `vault/raw_sync.rs` → `raw/code/`), never the live repo. The claude path hard-gates tools via `--tools` / `--allowedTools` / `--permission-mode acceptEdits` + PreToolUse hooks (`codebus hook check-bash` / `check-read`); the codex path uses the OS `-s` sandbox plus isolation flags (`--ignore-user-config --disable apps --ignore-rules -c project_root_markers=… -c windows.sandbox=unelevated -c web_search=disabled`). `docs/security.md` §5 holds the precise (Windows-non-admin, codex 0.135.0) read/write/egress + subagent posture; treat it as authoritative and keep it honest when changing isolation behavior.

## Non-obvious invariants

- **Cancellation / timeout** kill the whole process tree via `KillHandle::terminate_tree()` (Windows Job Object `KILL_ON_JOB_CLOSE` / Unix `killpg`). codex on Windows is a `.cmd` → `node.exe` → native `codex.exe` chain whose `codex.exe` leaf holds the inherited stdout pipe and can outlive the shim; killing only the immediate child leaves the pipe open (no EOF) and a naive pipe read hangs — cancellation must kill the whole tree, and ad-hoc codex probe output must be redirected to a file, not piped.
- **`docs/BACKLOG.md`** tracks open work and links to `docs/<date>-<slug>-backlog.md` detail docs; it is table-formatted and Git stores it as LF (`core.autocrlf=input`, despite occasional CRLF in the working tree). Be careful editing rows — removing one can mash adjacent rows, so grep for `|| 2026` after.
- This repo dogfoods itself: the `.codebus/` directory at the repo root is codebus's own vault, with materialized (generated) skill bundles and instruction files — not hand-authored source.

<!-- SPECTRA:START v1.0.2 -->

# Spectra Instructions

This project uses Spectra for Spec-Driven Development(SDD). Specs live in `openspec/specs/`, change proposals in `openspec/changes/`.

## Use `/spectra-*` skills when:

- A discussion needs structure before coding → `/spectra-discuss`
- User wants to plan, propose, or design a change → `/spectra-propose`
- Tasks are ready to implement → `/spectra-apply`
- There's an in-progress change to continue → `/spectra-ingest`
- User asks about specs or how something works → `/spectra-ask`
- Implementation is done → `/spectra-archive`
- Commit only files related to a specific change → `/spectra-commit`

## Workflow

discuss? → propose → apply ⇄ ingest → archive

- `discuss` is optional — skip if requirements are clear
- Requirements change mid-work? Plan mode → `ingest` → resume `apply`

## Parked Changes

Changes can be parked（暫存）— temporarily moved out of `openspec/changes/`. Parked changes won't appear in `spectra list` but can be found with `spectra list --parked`. To restore: `spectra unpark <name>`. The `/spectra-apply` and `/spectra-ingest` skills handle parked changes automatically.

<!-- SPECTRA:END -->
