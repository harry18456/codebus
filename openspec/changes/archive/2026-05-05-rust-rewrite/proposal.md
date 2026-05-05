## Why

CodeBus 的 long-term roadmap 已從「aspirational」升級為 day-1 committed：interactive tutorial 桌面 app 將以 Tauri 實作。在這個前提下繼續用 TypeScript 寫 CLI 會在未來 Tauri 整合時被迫選 sidecar 模式（包大小 +50-100MB、token streaming 多 4 個 IPC hop、cancel 信號跨進程）或最終仍要重寫一次（做兩次工）。0.1.0 尚未 release、`main` working tree clean、無 active spectra changes、無 user base — 這是重寫成本最低的時間窗。

## What Changes

- **BREAKING**：實作語言從 TypeScript 換成 Rust，distribution 形態從 npm package 改為 Rust binary（`cargo install` / GitHub Releases；npm 發行另開 spec）
- 採 Cargo workspace + 3 crate 架構：
  - `codebus-core/`：純 Rust lib，封裝 lint、frontmatter、stale-detect、page-merge、vault layout、stream parser、LLMProvider trait、I/O（fs、git、claude_cli subprocess）
  - `codebus-cli/`：clap 殼，引用 codebus-core，提供 `init` / `goal` / `query` / `check` 4 個 subcommand，args / stdout / exit code 與 TS 0.1.0 對齊
  - `codebus-app/`：預留 Tauri crate 位置（本 change 僅在 workspace 註冊、不實作內容）
- **吸收並取代** parked change `wiki-hygiene-signals`：page-size warn + unexpected-file warn 兩條 lint rule 併入 Rust `lint.rs` 初版
- Schema（agent system prompt）從 src/schema/claude-md.ts template literal 抽出成獨立 codebus-core/src/schema/CLAUDE.md，Rust 端 include_str! 引用；rewrite 開始前 TS 端先重構成讀同一檔，避免漂移
- 既有 TS code（src/、tests/）移入 legacy/ts-src/ 作為 reference impl 與 conformance baseline 來源；Phase D 達 CLI parity 後移除 legacy/
- 新增 conformance fixture：rewrite 開始前用 TS 對 D:/side_project/uv vault 跑所有 deterministic command，輸出冷凍進 tests/fixtures/uv-vault-snapshot/，Rust test 比 byte-equal
- 不改變 vault 格式（.codebus/wiki/** 結構保留，Rust 與 TS 寫的 vault 雙向相容）
- 不改變 CLI 行為契約（subcommand、args、stdout 格式、exit code、frontmatter / lint 輸出全部 byte-equal）

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `wiki-lint`：吸收 parked change `wiki-hygiene-signals` 的兩條 warn rule（page-size threshold per file type、unexpected-file detection），擴充「Lint emits warnings for structural and Obsidian-compatibility violations」requirement

## Impact

- Affected specs:
  - Modified: openspec/specs/wiki-lint/spec.md（page-size + unexpected-file warn rules）
- Affected code:
  - New: Cargo.toml
  - New: codebus-core/Cargo.toml
  - New: codebus-core/src/lib.rs
  - New: codebus-core/src/schema/CLAUDE.md
  - New: codebus-core/src/schema/mod.rs
  - New: codebus-core/src/wiki/mod.rs
  - New: codebus-core/src/wiki/types.rs
  - New: codebus-core/src/wiki/frontmatter.rs
  - New: codebus-core/src/wiki/date.rs
  - New: codebus-core/src/wiki/page_merge.rs
  - New: codebus-core/src/wiki/stale_detect.rs
  - New: codebus-core/src/wiki/lint.rs
  - New: codebus-core/src/vault/mod.rs
  - New: codebus-core/src/vault/layout.rs
  - New: codebus-core/src/vault/sanity_check.rs
  - New: codebus-core/src/vault/lock.rs
  - New: codebus-core/src/stream/mod.rs
  - New: codebus-core/src/stream/parser.rs
  - New: codebus-core/src/llm/mod.rs
  - New: codebus-core/src/llm/provider.rs
  - New: codebus-core/src/llm/claude_cli.rs
  - New: codebus-core/src/fs/mod.rs
  - New: codebus-core/src/fs/raw_sync.rs
  - New: codebus-core/src/fs/file_ops.rs
  - New: codebus-core/src/git/mod.rs
  - New: codebus-core/src/git/source_version.rs
  - New: codebus-core/src/git/nested_repo.rs
  - New: codebus-core/tests/conformance.rs
  - New: codebus-cli/Cargo.toml
  - New: codebus-cli/src/main.rs
  - New: codebus-cli/src/commands/mod.rs
  - New: codebus-cli/src/commands/init.rs
  - New: codebus-cli/src/commands/goal.rs
  - New: codebus-cli/src/commands/query.rs
  - New: codebus-cli/src/commands/check.rs
  - New: codebus-app/Cargo.toml
  - New: tests/fixtures/uv-vault-snapshot/
  - New: legacy/README.md
  - Modified: src/schema/claude-md.ts
  - Removed: legacy/ts-src/
  - Removed: package.json
  - Removed: tsconfig.json
  - Removed: node_modules/
