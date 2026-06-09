# Contributing to codebus

Thanks for taking a look. codebus is a Rust workspace (`codebus-core`,
`codebus-cli`, `codebus-app/src-tauri`) plus a React/Tauri desktop app
(`codebus-app`). Edition 2024, Rust 1.85+.

## Prerequisites

- Rust 1.85+ (edition 2024)
- Node 20+ (for the desktop app)
- At least one agent CLI for live/manual runs: [Claude Code](https://claude.ai/code) (default) or [OpenAI Codex](https://github.com/openai/codex). Unit/integration tests do **not** need a real agent — they drive a mock spawn binary.

## Build & test (Rust)

```bash
cargo build                      # whole workspace
cargo build -p codebus-core      # one crate

cargo test -p codebus-core       # core lib + integration tests
cargo test -p codebus-cli        # CLI integration tests (drive tests/bins/mock_claude.rs, no real agent)

# single test
cargo test -p codebus-core <name-substring>
cargo test -p codebus-cli --test <file> <name>     # e.g. --test quiz_flow

cargo clippy --workspace         # lint
cargo fmt --all                  # format
```

Live-agent tests (real Claude/Codex, spend API quota) are `#[ignore]` + env-gated
(e.g. `CODEBUS_LIVE_CODEX=1`, run with `-- --ignored`). Don't run them in normal CI/dev.

`cargo install --path codebus-cli` puts `codebus` on PATH — **required for the `fix` verb**,
which shells out to `codebus lint`.

## Build & test (desktop app)

```bash
cd codebus-app
npm install
npm run test         # Vitest
npm run typecheck    # tsc --noEmit
npm run tauri dev    # run the app
```

## Bar for changes

- **No new clippy warnings.** A small pre-existing baseline exists; the bar is *no new* warnings, not zero.
- Keep `cargo fmt` clean.
- Add/extend tests for behavior changes. The CLI tests use the mock spawn binary, not a real agent.

## Spec-driven workflow

codebus uses [Spectra](https://github.com/) spec-driven development. Specs live in
`openspec/specs/` (see [`openspec/specs/README.md`](openspec/specs/README.md)); change
proposals live in `openspec/changes/`, archived under `openspec/changes/archive/`.
Non-trivial changes should go through a proposal rather than landing as a bare diff.

## Where things live

- Architecture overview: [`docs/codebus-ai-architecture.md`](docs/codebus-ai-architecture.md)
- Security / isolation model: [`docs/security.md`](docs/security.md)
- Internal working notes / backlog: [`docs/internal/`](docs/internal/)

Issues, bug reports, and PRs welcome.
