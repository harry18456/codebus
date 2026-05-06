## 1. R1 — LLM domain：factory + providers/ subfolder

- [x] 1.1 [P] Write failing tests for `codebus-core/src/llm/factory.rs`：`build_provider(ProviderConfig { kind: ClaudeCli, ... })` 回 `Box<dyn LlmProvider>`、`ProviderKind::ClaudeCli` 對應 ClaudeCli impl、unknown kind 從 config.yaml 進來時 factory 回 `ProviderError::FeatureNotCompiled` 或 `ProviderError::Setup`
- [x] 1.2 Implement **Cargo workspace + 3 crate 結構** 下的 LLM factory：`codebus-core/src/llm/factory.rs` 新增 `ProviderKind` enum（含 `ClaudeCli` variant + 為未來 `AnthropicApi` / `OpenAi` / `OllamaLocal` 預留 #[cfg(feature)] gated variants）、`ProviderConfig` struct、`build_provider` 函式
- [x] 1.3 `git mv codebus-core/src/llm/claude_cli.rs codebus-core/src/llm/providers/claude_cli.rs`，新增 `codebus-core/src/llm/providers/mod.rs` re-export；改 `codebus-core/src/llm/mod.rs` 為新 layout（trait + factory 在頂層、impls 在 providers/）
- [x] 1.4 `codebus-cli/src/main.rs` 兩處 `ClaudeCliProvider::new()` 改 call factory：`build_provider(load_config().llm)` — 落地 **Register 機制：手動 factory match，不用 inventory / linkme**
- [x] 1.5 跑 `cargo test --workspace` 全綠（既有 121 個 codebus-core test + 15 個 cli test 不應 regress）
- [x] 1.6 跑 `target/release/codebus.exe --repo D:/side_project/buddy-gacha --check` 確認與 0.2.0 行為一致

## 2. R2 — PII domain：trait + factory + Null + RegexBasic

- [x] 2.1 [P] Write failing tests for `codebus-core/src/pii/scanners/null_scanner.rs`：`scan(&self, content, path)` 對任意輸入回空 `Vec<PiiMatch>`
- [x] 2.2 [P] Write failing tests for `codebus-core/src/pii/scanners/regex_basic.rs`：偵測常見 secret pattern（AWS access key `AKIA[A-Z0-9]{16}`、Anthropic API key `sk-ant-`、generic email、IPv4），各 pattern 一個 known-positive + 一個 known-negative case；可加 patterns_extra 並驗證自訂 pattern 觸發
- [x] 2.3 Implement `codebus-core/src/pii/{provider.rs,factory.rs,scanners/{null_scanner,regex_basic}.rs}`：定義 `PiiScanner` trait（**Trait sync/async：LLM async、其他 sync** — sync 版）、`PiiMatch` struct（`pattern_name`、`start`、`end`、`matched_text`、`severity`）、`ScannerKind` enum、`ScannerConfig` struct
- [x] 2.4 改 `codebus-core/src/lib.rs` 加 `pub mod pii;`
- [x] 2.5 跑 `cargo test --workspace` 全綠；確認 raw_sync 行為仍與 0.2.0 一致（因為本階段不接 scanner 進 raw_sync — 落地 **預設行為中性、僅一處 user-visible 改動**）

## 3. R3 — Lint domain：trait + 拆 6 條 rule 為獨立檔

- [x] 3.1 Implement `codebus-core/src/wiki/lint/rule.rs`：定義 `LintRule` trait（sync，落地 **Trait shape：object-safe + Box<dyn T>** 跟 **Trait 同步性：LLM async、其他 sync**）— `name(&self) -> &str`、`check(&self, ctx: &VaultContext) -> Vec<LintIssue>`；`VaultContext` struct 包 wiki_root、catalog（pre-computed page slugs + entries）、SPECIAL_FILES const
- [x] 3.2 Implement `codebus-core/src/wiki/lint/factory.rs`：`build_default_rules() -> Vec<Box<dyn LintRule>>` 回 6 條既有 rule
- [x] 3.3 [P] 把 `codebus-core/src/wiki/lint.rs` 的 page-size 邏輯拆到 `codebus-core/src/wiki/lint/rules/page_size.rs`，實作 `LintRule` trait
- [x] 3.4 [P] 拆 unexpected-file 到 `codebus-core/src/wiki/lint/rules/unexpected_file.rs`
- [x] 3.5 [P] 拆 duplicate-slug 到 `codebus-core/src/wiki/lint/rules/duplicate_slug.rs`
- [x] 3.6 [P] 拆 missing-nav 到 `codebus-core/src/wiki/lint/rules/missing_nav.rs`
- [x] 3.7 [P] 拆 root-page 到 `codebus-core/src/wiki/lint/rules/root_page.rs`
- [x] 3.8 [P] 拆 broken-wikilink + body-wikilink scan（含 markdown-aware code region 跳過、`\|` 處理）到 `codebus-core/src/wiki/lint/rules/broken_wikilink.rs`
- [x] 3.9 [P] 拆 frontmatter-integrity（parse 失敗、related[] 格式 / 解析）到 `codebus-core/src/wiki/lint/rules/frontmatter_integrity.rs`
- [x] 3.10 改 `lint_wiki()` 為新形態：載入 `Vec<Box<dyn LintRule>>`、iterate 跑每條、彙總 issues；保留 pages_scanned / nav_files_scanned 統計（不屬任何 rule、留在 lint_wiki 主體）
- [x] 3.11 刪除原 `codebus-core/src/wiki/lint.rs`、新增 `codebus-core/src/wiki/lint/mod.rs` re-export
- [x] 3.12 跑既有 lint tests 全綠（特別 `lint_uv_fixture_produces_known_warning_count` 對 uv vault 的 struct-level conformance）
- [x] 3.13 跑 `target/release/codebus.exe --repo D:/side_project/uv --check` 與 `tests/fixtures/uv-vault-snapshot/check-output.txt` byte-equal

## 4. R4 — Render domain：EventRenderer trait + Terminal impl

- [x] 4.1 Implement `codebus-core/src/render/event_renderer.rs`：定義 `EventRenderer` trait（sync）— `render(&mut self, event: &StreamEvent)`、`render_banner(&mut self, banner: &Banner)`、`flush(&mut self)` default no-op
- [x] 4.2 Implement `codebus-core/src/render/factory.rs`：`RendererKind` enum (`Terminal` 為 day-1 唯一，預留 `JsonLines` 跟 `Tauri` variants)、`build_renderer`
- [x] 4.3 Implement `codebus-core/src/render/renderers/terminal.rs`：把 `codebus-cli/src/ui.rs::render_event` + `render_banner` + `print_lint_report` + `format_lint_summary` 整段邏輯搬進來、改實作 `EventRenderer` trait；保留 `RenderOptions` 概念但內化為 TerminalRenderer 自己的 field
- [x] 4.4 改 `codebus-core/src/lib.rs` 加 `pub mod render;`
- [x] 4.5 改 `codebus-cli/src/commands/{goal,query}.rs`：on_event callback 改成 `&mut dyn EventRenderer`；main.rs build renderer 後傳入
- [x] 4.6 改 `codebus-cli/src/commands/check.rs`：lint output 改透過 `EventRenderer` 而非直接 print — 落地 **EventRenderer trait + Terminal impl 抽出**
- [x] 4.7 跑 fixture byte-equal smoke：`uv vault --check` stdout 仍與 `check-output.txt` byte-equal
- [x] 4.8 跑 buddy-gacha smoke：`init` / `goal` / `check` 三條路徑與 0.2.0 視覺一致

## 5. R5 — Log domain：LogSink trait + Null + JsonLines

- [x] 5.1 [P] Write failing tests for `codebus-core/src/log/sinks/null_sink.rs`：`write_run` / `write_token_usage` 都是 no-op
- [x] 5.2 [P] Write failing tests for `codebus-core/src/log/sinks/jsonl_sink.rs`：append `RunLog` 到 `<dir>/<YYYY-MM-DD>.jsonl`、檔案不存在會自動建、format 是 jsonl（每筆一行 valid JSON）
- [x] 5.3 Implement `codebus-core/src/log/sink.rs`：定義 `LogSink` trait（sync）、`RunLog` struct（goal text、started_at、finished_at、tokens、wiki_changed、lint summary）、`TokenUsage` struct
- [x] 5.4 Implement `codebus-core/src/log/factory.rs`、`codebus-core/src/log/sinks/{null_sink,jsonl_sink}.rs`
- [x] 5.5 改 `codebus-core/src/lib.rs` 加 `pub mod log;`
- [x] 5.6 `codebus-cli/src/commands/{goal,query}.rs` 接 `&mut dyn LogSink` 參數、預設 `NullSink` — 行為與 0.2.0 一致（落地 **不啟用 LogSink 寫檔** Non-Goal）
- [x] 5.7 跑 `cargo test --workspace` 全綠

## 6. R6 — Config domain：unified config.yaml

- [x] 6.1 Implement `codebus-core/src/config/schema.rs`：定義 `GlobalConfig` struct（含 `emoji`、`llm`、`pii`、`lint`、`render`、`log` 五個 plugin section + `emoji` field），所有欄位 `#[serde(default)]`、所有 plugin section 內部子欄位也 `#[serde(default)]`
- [x] 6.2 Implement `codebus-core/src/config/loader.rs`：`load_config()` 讀 `~/.codebus/config.yaml`（用 `dirs` crate 找 home），不存在回 `GlobalConfig::default()`、parse 失敗 warn-but-not-abort、unknown discriminator warn 但 section treated as unset；落地 spec MODIFIED requirement 「Load global config tolerantly」全 12 個 scenarios
- [x] 6.3 Write tests for config loader 對應每一個 spec scenario：missing file / invalid yaml / unknown emoji / future top-level field / llm provider discriminator / unknown llm provider / unknown sub-field / pii scanner / lint disabled rules / render format / log sink / empty plugin section / type-mismatched sub-field
- [x] 6.4 改 `codebus-core/src/lib.rs` 加 `pub mod config;`、`codebus-cli/Cargo.toml` 加 `serde_yaml` dep
- [x] 6.5 改 `codebus-cli/src/main.rs`：startup 時 `let cfg = load_config()`、merge 進 `RenderOptions`（emoji 5 級優先序：CLI flag > `--no-emoji` > `NO_EMOJI` env > config.yaml > auto） + 各 plugin factory 的 input config — 落地 **統一 `~/.codebus/config.yaml` schema**
- [x] 6.6 加 integration test：寫 `emoji: off` 到 tmp config.yaml、設 `HOME` env 指過去、跑 codebus、stdout 不含 emoji
- [x] 6.7 跑 `target/release/codebus.exe --repo D:/side_project/uv --check` 仍與 `tests/fixtures/uv-vault-snapshot/check-output.txt` byte-equal（user 沒設 config.yaml 時走 auto，與 0.2.0 一致）

## 7. R7 — Cargo features：lean default + heavy gated

- [x] 7.1 改 `codebus-core/Cargo.toml`：default features 為空；加 feature gates `pii-presidio`、`pii-aws`、`llm-anthropic-api`、`llm-openai`、`log-otel`；加便利集合 `all-llm`、`all-pii`、`all` — 落地 **Cargo features：default lean、heavy dep 切 feature** 與 **Crate 結構：5 個 plugin domain 全在 codebus-core**（驗證所有 plugin 都在 codebus-core/Cargo.toml 而非拆出 sub-crate）
- [x] 7.2 確認當前 codebase 在 `cargo check --workspace --no-default-features` 全綠（驗 default 真的 lean）
- [x] 7.3 確認 `cargo check --workspace --all-features` 編譯成功（即使裡面 feature variant 都還沒實作 — 預留 #[cfg(feature)] gated `unimplemented!()` stub 即可）
- [x] 7.4 README.md 加一段 install variants：`cargo install codebus`（lean）vs `cargo install codebus --features all-llm,pii-presidio`（fat）

## 8. R8 — Acceptance + commit + cool-down

- [x] 8.1 跑全套 Phase A / B / C 既有 conformance gates：`cargo test --workspace`、`cargo llvm-cov --workspace` ≥ 80%、`cargo clippy --workspace -- -D warnings`、`cargo fmt --all -- --check`
- [x] 8.2 對 `D:/side_project/uv` 跑 `--check` byte-equal
- [x] 8.3 對 `D:/side_project/buddy-gacha` 跑完整 init + goal + check 流程驗 e2e 視覺與 0.2.0 一致
- [x] 8.4 寫一筆 `~/.codebus/config.yaml` 含 5 個 plugin section + emoji 設定，跑 codebus 確認每個 section 都被讀取（debug log 或加 `--debug` flag 印 effective config）
- [ ] 8.5 Cool-down 一週 — 期間用 codebus 跑自己的探索任務、發現 plugin 介面 friction 就回頭調
- [x] 8.6 Final commit：`refactor(plugin-architecture): 5 plugin domains + config.yaml restored`
- [ ] 8.7 `spectra archive plugin-architecture-refactor`；如過程中任一 R 階段 fail，依 design 的 **Rollback 策略** 用 `git reset --hard HEAD~1` 回前一個 R checkpoint，整個 change 失敗則回 `267b3c5`（rust-rewrite 完成 commit）
