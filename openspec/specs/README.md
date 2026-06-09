# codebus capability specs

codebus is developed spec-first. Each subdirectory here is one **capability** with a
`spec.md` describing its requirements and scenarios. Change proposals (in
`../changes/`, archived under `../changes/archive/`) are what edit these specs over time.

This is the living, current spec set — a good map of what codebus actually guarantees.

## Agent & execution core

- [`agent-backend`](agent-backend/spec.md) — provider-neutral agent seam (the `AgentBackend` trait + invocation loop)
- [`codex-backend`](codex-backend/spec.md) — OpenAI Codex backend (sandbox + isolation flags, Azure)
- [`verb-library`](verb-library/spec.md) — the `goal`/`query`/`fix`/`chat`/`quiz` verb layer
- [`agent-stream-rendering`](agent-stream-rendering/spec.md) — provider JSONL → neutral stream events
- [`run-log`](run-log/spec.md) — per-invocation run-log rows
- [`events-log`](events-log/spec.md) — per-run event JSONL

## Verbs & vault features

- [`vault`](vault/spec.md) — the `.codebus/` vault layout + raw source mirror
- [`pii-filter`](pii-filter/spec.md) — PII scanner + redaction on the source mirror
- [`chat-verb`](chat-verb/spec.md) — multi-turn chat REPL
- [`quiz`](quiz/spec.md) — quiz generation / attempt flow
- [`lint-feedback-loop`](lint-feedback-loop/spec.md) — `lint` rules + `fix` self-repair loop
- [`skill-bundles`](skill-bundles/spec.md) — materialized per-verb SKILL.md instruction bundles
- [`fs-watcher`](fs-watcher/spec.md) — filesystem watcher for vault/lobby changes

## Configuration & CLI

- [`cli`](cli/spec.md) — CLI command surface
- [`claude-code-config`](claude-code-config/spec.md) — claude provider config + keyring
- [`codex-config`](codex-config/spec.md) — codex provider config

## Desktop app

- [`app-shell`](app-shell/spec.md) — app shell / lobby
- [`app-workspace`](app-workspace/spec.md) — workspace (goal / chat / quiz / wiki tabs)
- [`design-system`](design-system/spec.md) — UI design tokens / components

## Packaging

- [`release-automation`](release-automation/spec.md) — release pipeline
- [`windows-distribution`](windows-distribution/spec.md) — Windows installer distribution
