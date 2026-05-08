# uv-vault-snapshot

Conformance baseline for the TypeScript → Rust rewrite. Frozen output of the TS 0.1.0 binary against `D:/side_project/uv` so the Rust impl can be diffed for behavioral parity.

Captured: 2026-05-05 (TS 0.1.0)

## Contents

| Path | What it is |
|---|---|
| `check-output.txt` | stdout of `node dist/cli.js --repo D:/side_project/uv --check` |
| `check-exit-code.txt` | exit code of the same command |
| `uv-wiki-snapshot/` | full copy of `D:/side_project/uv/.codebus/wiki/` at snapshot time, including `.obsidian/`, all 5 type folders, `goals/`, and root nav files (`index.md`, `log.md`, `overview.md`) |

## How Rust uses this

- **frontmatter parser** (codebus-core/src/wiki/frontmatter.rs): each `.md` file under `uv-wiki-snapshot/{concepts,entities,modules,processes,synthesis}/` is a parser fixture case. Parser must produce the same `Frontmatter` struct shape as the TS `parsePage` function. Compare via byte-equal serialization or struct-level equality.
- **lint** (codebus-core/src/wiki/lint.rs): point lint at `uv-wiki-snapshot/` and compare stdout to `check-output.txt` byte-equal. Exit code from check must match `check-exit-code.txt`.
- **Phase A conformance gate** (task 2.23): pure-module Rust tests pass byte-equal against this fixture.
- **Phase C conformance gate** (task 4.10): `codebus-cli` `check` subcommand pointed at `uv-wiki-snapshot/` reproduces `check-output.txt` exactly.

## What's intentionally NOT snapshot

The fixture covers only **deterministic** behaviors:

- `init` is not snapshot here. Init creates a fixed `.codebus/` skeleton + does gitignore-aware `raw-sync` from the source repo. Init parity is verified by direct Rust unit tests against the schema constant (`codebus-core/src/schema/CLAUDE.md`) and a small synthetic source-repo fixture, not against this uv snapshot.
- `query` calls the LLM provider — non-deterministic stream output. Rust tests use a mock `LLMProvider` that yields a fixed `StreamEvent` sequence; orchestration side-effects (stdout render, lint-report format) are byte-fixtured separately.
- `goal` likewise calls the LLM provider. Mocked the same way as `query`. Orchestration deterministic side-effects (goals.jsonl entry, enrichSourceMetadata, autoCommit message) are unit-tested against in-memory state, not against this fixture.

This scoping is consistent with the design doc's Conformance decision: "fixture 比對對 deterministic 部分超有效。對非 deterministic 部分（goal 命令呼叫 LLM）沒用 — LLM 每次回答不一樣。"

## Regenerating

If the TS impl changes intentionally during the rewrite (e.g., schema edit while still in TS), regenerate this fixture with the latest TS binary:

```
npm run build
node dist/cli.js --repo D:/side_project/uv --check > tests/fixtures/uv-vault-snapshot/check-output.txt
echo $? > tests/fixtures/uv-vault-snapshot/check-exit-code.txt
rm -rf tests/fixtures/uv-vault-snapshot/uv-wiki-snapshot
cp -r D:/side_project/uv/.codebus/wiki tests/fixtures/uv-vault-snapshot/uv-wiki-snapshot
```

Commit the diff with a message documenting the behavioral change. After Phase D removes the TS binary, this fixture is frozen and only updated if Rust intentionally diverges (with diff visible in PR).
