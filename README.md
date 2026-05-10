# codebus

Help engineers ramp up on unfamiliar codebases by building an AI-curated, Obsidian-compatible markdown wiki.

## Status

v3 main sequence shipped (init / goal / query / lint / fix verbs + PII / config / banner UX). v3.0.0-dev binary lives at `target/release/codebus.exe` after `cargo build --release`. Roadmap follow-ups in `docs/v3-roadmap.md` §4.

- **`v2-archive`** branch — frozen v2 Rust workspace (full git history, all archived spec changes preserved)
- `legacy/v2-rust/` — v2 reference impl in working tree
- `legacy/ts-src/` — v1 TypeScript prototype reference
- The strategic discussion that led to this fresh start lives at `legacy/v2-rust/docs/strategy/2026-05-08-skill-vs-binary-pivot.md`

## Quickstart

### Build & install

```bash
# from repo root
cargo install --path codebus-cli
```

`cargo install` puts `codebus` on your `PATH` (under `~/.cargo/bin/`). **This is required for the `fix` verb to function correctly** — see "Why install on PATH" below. If you only want to try the binary without installing, `cargo build --release` produces `target/release/codebus.exe` which works for `init` / `goal` / `query` / `lint` invoked by absolute path; `fix` will degrade gracefully but loses its in-session lint feedback loop.

### First run

```bash
cd /path/to/your/repo
codebus init           # create .codebus/ vault, register with Obsidian (if installed)
codebus goal "..."     # spawn the codebus-goal agent against your vault
codebus query "..."    # ask the wiki a question (read-only)
codebus lint           # validate wiki/ structure
codebus fix            # auto-repair lint issues via agent
```

### Why install on PATH

The `fix` verb spawns a Claude Code child process whose Bash tool is gated to `Bash(codebus lint *)` only. The child agent uses this to iteratively re-lint after each repair edit, converging on a clean vault before terminating. If `codebus` is not resolvable on the spawned subprocess's `PATH`, the agent's `codebus lint` invocations fail (the agent reports "command not available") and loses its mid-session feedback signal. The CLI still runs a final lint check after the agent exits, so end-state correctness is preserved — but the agent's in-session iteration quality degrades significantly without `codebus` on `PATH`.

## License

[MIT](LICENSE)
