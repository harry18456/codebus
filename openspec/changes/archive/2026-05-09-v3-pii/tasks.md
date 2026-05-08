## 1. PII 模組骨架

- [x] 1.1 在 `codebus-core/src/pii/mod.rs` 建立模組入口，宣告 `provider`、`scanners` 兩個子模組（落實 PII Match Output Shape requirement 的型別容身）
- [x] 1.2 在 `codebus-core/src/pii/provider.rs` 定義 `PiiScanner` trait（`name(&self) -> &str` + `scan(&self, content: &str, path: &str) -> Vec<PiiMatch>`，trait bound `Send + Sync`）+ `PiiMatch` struct（`pattern_name` / `start` / `end` / `matched_text` / `severity`）+ `PiiSeverity` 二值 enum（`Critical` / `Warn`）+ `OnHit` 三值 enum（`Warn` / `Skip` / `Mask`，`#[derive(Default)]` + `#[default] Warn`），落實 PII Match Output Shape requirement 與 On-Hit Policy Default requirement
- [x] 1.3 在 `codebus-core/src/lib.rs` 加 `pub mod pii;` export 模組

## 2. Scanner 兩個 impl（並行）

- [x] [P] 2.1 在 `codebus-core/src/pii/scanners/null_scanner.rs` 實作 `NullScanner` struct + `impl PiiScanner for NullScanner`（`scan` 永遠回 `Vec::new()`、`name` 回 `"null"`）+ unit tests（clean input、含 `AKIAIOSFODNN7EXAMPLE` 仍 empty、空 input、object-safe `Box<dyn PiiScanner>`），落實 Null Scanner Behavior requirement
- [x] [P] 2.2 在 `codebus-core/src/pii/scanners/regex_basic.rs` 定義 `BUILTIN_PATTERNS` const slice 收 4 條 v2 carry regex（`aws-access-key` AKIA/ASIA + 16 alnum 為 `Critical`、`anthropic-api-key` `sk-ant-` + 20+ URL-safe 為 `Critical`、`email` 帶 dot TLD 為 `Warn`、`ipv4` 4 段 dotted-quad 為 `Warn`） + 實作 `RegexBasicScanner::new(patterns_extra: &[String]) -> Result<Self, regex::Error>`（builtin + extra 各自編譯，extra fail-fast）+ `impl PiiScanner for RegexBasicScanner`（scan 對所有 rule 跑 `find_iter` 回 sorted-by-offset Vec<PiiMatch>）+ unit tests 覆蓋 4 patterns positive、AWS 15-char negative、email no-TLD negative、version-string negative、malformed extra fail-fast、ascending offset sort，落實 Built-in Regex Pattern Coverage requirement 與 PII Match Output Shape 的 match-ordering scenario

## 3. Scanners 模組整合

- [x] 3.1 在 `codebus-core/src/pii/scanners/mod.rs` 宣告並 export `null_scanner` + `regex_basic` 兩個子模組
- [x] 3.2 在 `codebus-core/Cargo.toml` 確認 `regex` dependency 已在；若無，新增（v2 用版本範圍對齊）

## 4. raw_sync 接 PII Scanner

- [x] 4.1 在 `codebus-core/src/vault/raw_sync.rs` 把函數 `sync_with_null_scanner` 重命名為 `sync_with_scanner`，新增 `scanner: &dyn PiiScanner` 參數；呼應 design.md「函數重命名：`sync_with_null_scanner` → `sync_with_scanner`」決策，並落實「Scanner 構造位置：raw_sync 內部 hardcode」決策中 raw_sync 不自選 default 的邊界。本 task 是把現有 vault spec 的「Raw Mirror with NullScanner」requirement 切到新行為（archive 時搭配 RENAMED 規則改名為「Raw Mirror with PII Scanner」）
- [x] 4.2 在 `codebus-core/src/vault/raw_sync.rs` 把 `SyncSummary` 加 `pii_matches: usize` 欄位（保留 `Default` derive），呼應 design.md「`SyncSummary` 加 `pii_matches: usize` 欄位」決策
- [x] 4.3 在 mirror 主迴圈：對每個檔案讀內容、呼叫 `scanner.scan(&content, &rel_path_str)`、命中即時 `eprintln!("pii warn: {} at {}:{}", m.pattern_name, rel_path_str, m.start)`（不輸出 `matched_text`）、`summary.pii_matches += matches.len()`；命中後檔案仍 `fs::copy` 寫入（OnHit::Warn 行為），落實 Raw Mirror with PII Scanner requirement 中 stderr / 仍 mirror / 多 match 多行 / omit matched text 等 scenario，呼應 design.md「stderr warn 格式：每個 match 一行，**不印 matched_text**」決策
- [x] 4.4 在 `codebus-core/src/vault/raw_sync.rs` 補 unit tests：注入 `NullScanner` 跑 happy-path 驗 `pii_matches == 0`、注入 `RegexBasicScanner::new(&[])` 餵假檔含 AKIA shape 驗 stderr 命中行格式、檔案仍存在、`SyncSummary.pii_matches` 累加正確；呼應 design.md「NullScanner carry：trait 二 impl 兼 test fixture」決策，並落實 Raw Mirror with PII Scanner requirement 的 summary aggregate scenario

## 5. init 接 RegexBasic 並印 PII count

- [x] 5.1 在 `codebus-cli/src/commands/init.rs` 把 `sync_with_null_scanner(repo, &paths.raw_code)` 改為構造 `let scanner = RegexBasicScanner::new(&[]).map_err(|e| ...)?;` 後呼叫 `sync_with_scanner(repo, &paths.raw_code, &scanner)`；regex 編譯失敗時印 stderr error 並 `ExitCode::from(1)`，呼應 design.md「不 carry `ScannerConfig` / `build_scanner` / `on_hit_serde`」決策的 hardcode 路徑與 regex 編譯風險
- [x] 5.2 在 `codebus-cli/src/commands/init.rs` 把 raw mirror 進度行 `println!` 字面值改成 `"✓ raw mirror: {} files, {} bytes, {} PII matches"` 並餵入 `summary.pii_matches`，落實 Init Subcommand Behavior requirement 的 raw mirror PII match count scenario（含 zero-count case）

## 6. 整合測試與既有 test 對齊

- [x] 6.1 在 `codebus-core/tests/vault_init.rs` 把所有對 `sync_with_null_scanner` 的呼叫改成 `sync_with_scanner(..., &NullScanner::new())`，並把 `SyncSummary` 對應斷言補上 `pii_matches: 0` 欄位
- [x] 6.2 在 `codebus-core/tests/vault_init.rs` 新增整合 test `raw_sync_emits_warnings_for_known_pii_patterns`：建構假 repo 含 `src/aws.py` 帶 `AKIAIOSFODNN7EXAMPLE`、`docs/contact.md` 帶 `alice@example.com`、`docs/net.md` 帶 `192.168.1.42`，跑 `sync_with_scanner` + `RegexBasicScanner::new(&[])`，斷言 stderr 三行皆以 `pii warn:` 開頭、含對應 `pattern_name` + 路徑、不含 `matched_text` 字面值（`AKIA...` / `alice@...` / `192.168.1.42` 均不在 stderr）、檔案於 `.codebus/raw/code/` 內仍存在、`SyncSummary.pii_matches == 3`，落實 Raw Mirror with PII Scanner requirement 中 stderr / matched-text omission / multiple matches / summary aggregate 多個 scenario
- [x] 6.3 在 `codebus-cli/tests/cli_routing.rs`（或新增 init smoke test 檔）新增測試：zero-PII repo 跑 `codebus init` 斷言 stdout 進度行含 `"0 PII matches"`、含 PII repo 斷言 stdout 進度行含 `"<N> PII matches"` 且 `<N> > 0`，落實 Init Subcommand Behavior requirement 的 zero / non-zero PII count scenario
- [x] 6.4 跑 `cargo test -p codebus-core -p codebus-cli` 全綠
