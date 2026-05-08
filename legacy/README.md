# legacy/ts-src — TypeScript reference impl

Frozen TypeScript 0.1.0 implementation, kept here during the Rust rewrite (`openspec/changes/rust-rewrite`) as **reference only**.

## Status

- **Do NOT execute**: this code is not built, not tested, not shipped.
- **Do NOT modify**: edits here serve no purpose; Rust impl is the active codebase.
- **Do mine for context**: when Rust impl needs to verify behavior, grep this folder to see how the TS version handled the same edge case. The hard-won correctness from iter-8 / iter-9 (sandbox argv, stream-parser schema, enrichSourceMetadata invariant) lives in the comments and tests here.

## Contents

| Path | Original location |
|---|---|
| `src/` | was `<root>/src/` (28 source files, hexagonal core/infra/ui/commands) |
| `tests/` | was `<root>/tests/` minus `fixtures/`, which stayed at `<root>/tests/fixtures/` to serve as cross-language conformance baseline for Rust |

## Removal

This entire directory is deleted in tasks.md task 5.3 (Phase D cleanup), after:

1. All conformance gates green (Phase A 2.23, Phase B 3.12, Phase C 4.10)
2. Cool-down period of 1 week with smoke testing against real repos (task 5.2)
3. No user-facing behavior diff observed during cool-down

When that's done, `git rm -r legacy/` removes this folder. Git history retains the full TS implementation should it ever be needed for forensics.
