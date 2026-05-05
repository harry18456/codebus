## 1. Pre-rewrite preparation

- [x] 1.1 Snapshot TS conformance fixture：在 main 凍結 TS 之前，跑 `npm run build` + 對 `D:/side_project/uv` 各跑一次 `init` / `check` / `query`，把 stdout / exit code / 完整 vault 樹（含 frontmatter）全部冷凍進 `tests/fixtures/uv-vault-snapshot/`，作為 **Conformance 透過 fixture snapshot 確保行為一致** 的 baseline；fixture size 若超過 5MB 改用 git-lfs（一次性決定）
- [x] 1.2 Extract schema to standalone .md：把 `src/schema/claude-md.ts` 的 `CODEBUS_SCHEMA_MARKDOWN` template literal 抽出到 `codebus-core/src/schema/CLAUDE.md`，TS 端先改成 `readFileSync` 讀同一檔案（rewrite 期間共用），跑 152 個 TS test 確認無 regression — 落地 **Schema 拆出獨立 .md，Rust 與 TS 共用**
- [x] 1.3 Move legacy TS code：`git mv src/ legacy/ts-src/src/`、`git mv tests/ legacy/ts-src/tests/`、寫 `legacy/README.md` 註明「reference impl, do not execute」用途，落地 **既有 TS code 移入 legacy/ts-src/，Phase D 達 parity 後刪**
- [x] 1.4 Initialize Cargo workspace：建 root `Cargo.toml` 註冊三個 member（`codebus-core`、`codebus-cli`、`codebus-app`），三個資料夾各自 `Cargo.toml` + `src/lib.rs` 或 `src/main.rs` 空殼；`codebus-app/` 內僅 placeholder 印一行字（**Tauri app 留作獨立 spec（codebus-app/ 本 change 只預留位置）**），跑 `cargo check --workspace` 確認解析成功；落地 **Cargo workspace + 3 crate 結構**
- [x] 1.5 Pre-rewrite commit：完成 **Pre-rewrite checklist (Day 0)** 後 1 個 commit 把 1.1-1.4 的所有改動 push 上 main（**在 main 直接重寫，不開平行樹**），commit message 標明「pre-rewrite: snapshot fixture, move legacy, init cargo workspace」；commit 前確認 **Rollback 策略** 仍可用：`git reset --hard HEAD~1` 即恢復前一狀態（src/ tests/ package.json 全部還在 legacy/ 之前的位置）

## 2. Phase A — codebus-core pure modules

- [x] 2.1 Define core data types in `codebus-core/src/wiki/types.rs`：Page、Frontmatter（含 sources []SourceRef、related []String、tags []String、stale bool、created/updated UTC string）、SourceRef、LintIssue、LintResult，加 serde Serialize/Deserialize derive
- [x] 2.2 Schema include_str! lock-in：`codebus-core/src/schema/mod.rs` `pub const CODEBUS_SCHEMA: &str = include_str!("./CLAUDE.md");`，加幾個 `assert!(SCHEMA.contains("..."))` 的 lock-in test 鎖住關鍵字串（對應 TS `tests/schema/claude-md.test.ts`）
- [x] 2.3 [P] Write failing tests for date module（`utc_today_iso()` 回傳 `YYYY-MM-DD`，verify cross-timezone determinism）
- [x] 2.4 [P] Implement `codebus-core/src/wiki/date.rs`
- [x] 2.5 [P] Write failing tests for frontmatter parse/serialize against `tests/fixtures/uv-vault-snapshot/` 各 page 的 frontmatter（含 multi-line scalar、nested `sources[]`、broken cases 應 fail-soft）
- [x] 2.6 [P] Implement `codebus-core/src/wiki/frontmatter.rs` 用 serde_yaml + 自刻 `---` split helper
- [x] 2.7 [P] Write failing tests for page-merge（同名 page 合併、不同 frontmatter 處理規則）
- [x] 2.8 [P] Implement `codebus-core/src/wiki/page_merge.rs`
- [x] 2.9 [P] Write failing tests for stale-detect（compare frontmatter `sources[].sha256` vs current raw hash map，verify「全部一致 → not stale」「任一不一致 → stale」）
- [x] 2.10 [P] Implement `codebus-core/src/wiki/stale_detect.rs`
- [x] 2.11 [P] Write failing tests for vault layout（`vault_paths(repo)` 各路徑唯一 source of truth、wiki_page_folders 與 wiki_type_folder_map 一致）
- [x] 2.12 [P] Implement `codebus-core/src/vault/layout.rs`
- [x] 2.13 [P] Write failing tests for vault sanity-check（拒絕 `--repo .codebus/`、`--repo` 指向 vault 內部、`--repo` 指向不存在路徑）
- [x] 2.14 [P] Implement `codebus-core/src/vault/sanity_check.rs`
- [x] 2.15 [P] Write failing tests for vault lock acquisition / release / stale-lock cleanup
- [x] 2.16 [P] Implement `codebus-core/src/vault/lock.rs`
- [x] 2.17 Write failing tests for stream parser：StreamEvent enum（thought / tool_use / tool_result / done）+ iter-8 schema 真相驗證（`{type:"assistant",message:{content:[...]}}`、`assistant.content[]` 多元素、unknown event 靜默丟棄不 throw）
- [x] 2.18 Implement `codebus-core/src/stream/parser.rs`
- [x] 2.19 Define LLMProvider trait in `codebus-core/src/llm/provider.rs`：`async fn invoke(opts: InvokeOptions) -> impl Stream<Item = StreamEvent>` + `fn cancel()`，落地 **LLMProvider trait 保持單一 ClaudeCli 實作（Phase 2 抽象延後）**（trait 形狀對齊 TS 0.1.0、本 change 只實作 ClaudeCli）
- [x] 2.20 Write failing tests for lint covering all rules of "Lint emits warnings for structural and Obsidian-compatibility violations"：既有 4 條（root page、duplicate slug、missing nav、broken body wikilink）+ 2 條新 rule（page-size 6 個 scenario、unexpected-file 4 個 scenario）對應 **吸收 wiki-hygiene-signals 兩條 lint rule**
- [x] 2.21 Implement `codebus-core/src/wiki/lint.rs` 依序：(a) catalog（5 type folder + index/log）、(b) duplicate slug scan、(c) missing nav scan、(d) page-size scan（per-folder threshold strict greater-than，message 含 `size N bytes` + `threshold M bytes`）、(e) unexpected-file scan（hidden 排除、unrecognized folder、nested sub-folder、non-.md file）、(f) frontmatter integrity + related[] wikilink validation、(g) body wikilink scan（含 markdown-aware code region 跳過、`\|` 處理）
- [x] 2.22 Phase A coverage gate：`cargo llvm-cov` 全 codebus-core pure 模組報 ≥ 80%；確認 **Phase 順序：core pure → core I/O → CLI → cleanup** 第一段達標
- [x] 2.23 Phase A conformance gate：跑 `tests/fixtures/uv-vault-snapshot/` 中所有 deterministic frontmatter / lint case，Rust 輸出與 TS baseline byte-equal

## 3. Phase B — codebus-core I/O modules

- [ ] 3.1 Spike git2 vs `std::process::Command` for git operations：1 day timebox，跑通 `git status --porcelain`、`git rev-parse HEAD`、`git commit` 三條路徑各兩種實作，記錄 lifetime 複雜度 / API 表面差異 / Windows path 行為，挑一條進入 3.6-3.9
- [ ] 3.2 [P] Write failing tests for `fs/file_ops`（sha256_file 對 known input 回傳 known hex digest）
- [ ] 3.3 [P] Implement `codebus-core/src/fs/file_ops.rs`
- [ ] 3.4 [P] Write failing tests for `fs/raw_sync`（gitignore-aware copy 跳過 `node_modules/`、`.git/`、custom `.gitignore` 規則）
- [ ] 3.5 [P] Implement `codebus-core/src/fs/raw_sync.rs` 用 `ignore` crate（ripgrep 用的）
- [ ] 3.6 [P] Write failing tests for `git/source_version`（commit hash + uncommitted boolean）
- [ ] 3.7 [P] Implement `codebus-core/src/git/source_version.rs`
- [ ] 3.8 [P] Write failing tests for `git/nested_repo`（auto-commit 一次 message、兩次 commit 之間 working tree 為空時不 commit）
- [ ] 3.9 [P] Implement `codebus-core/src/git/nested_repo.rs`
- [ ] 3.10 Write failing tests for `llm/claude_cli`：spawn 用 mock SpawnFn 注入、確認 argv 嚴格 sandbox（**雙重斷言**：`--tools Read,Glob,Grep[+Write,Edit if ingest]` 帶到 + 同一 list 出現在 `--allowedTools` + 「未授權 tool 名（Bash、WebFetch、TodoWrite 等）絕對不在 argv 任何位置」）— 重新驗證 iter-9 教訓；測 stream-json input + stream parsing roundtrip + classify_exit 三類 verdict
- [ ] 3.11 Implement `codebus-core/src/llm/claude_cli.rs` 用 **Async runtime 選 tokio**（`tokio::process::Command` + `tokio::io::BufReader::lines`）
- [ ] 3.12 Phase B coverage gate：cargo llvm-cov 報 codebus-core 整體 ≥ 80%

## 4. Phase C — codebus-cli

- [ ] 4.1 [P] Write failing test for `init` command parity：對空 repo 跑 init，比對產出的 `.codebus/wiki/` 樹與 fixture `wiki-after-init/` byte-equal（用 mock LLM provider 模擬「無 LLM 呼叫」初始化路徑）
- [ ] 4.2 [P] Implement `codebus-cli/src/commands/init.rs`
- [ ] 4.3 [P] Write failing test for `check` command parity：對 fixture vault 跑 `--check`，stdout 與 `check-output.txt` byte-equal、exit code 與 `check-exit-code.txt` 一致（含 page-size + unexpected-file warnings 全部出現）
- [ ] 4.4 [P] Implement `codebus-cli/src/commands/check.rs`
- [ ] 4.5 Write failing test for `query` command parity：mock LLMProvider 回固定 StreamEvent 序列，比對 stdout render 結果（emoji-mode 5-level + lint-report 格式）byte-equal
- [ ] 4.6 Implement `codebus-cli/src/commands/query.rs`（含 ui/render、ui/emoji-mode、ui/lint-report 對應 Rust port）
- [ ] 4.7 Write failing test for `goal` command orchestration：mock LLMProvider，verify (a) goals.jsonl entry append、(b) raw_sync 觸發、(c) enrichSourceMetadata 只填 missing sources（**保留 iter-8 invariant：已 enriched 的 page 不重算 sha256**）、(d) flagStalePages run、(e) lintWiki 在 autoCommit 前跑、(f) autoCommit message 格式
- [ ] 4.8 Implement `codebus-cli/src/commands/goal.rs`
- [ ] 4.9 Implement `codebus-cli/src/main.rs`：clap subcommand entry（init / goal / query / check）、SIGINT handler（順序 trap：先讀 `opts.repo` 再 install handler，避免 iter-8 TDZ trap 的 Rust 對應）、global flags（`--repo`、`--check`）
- [ ] 4.10 Phase C conformance gate：跑完整 `tests/fixtures/uv-vault-snapshot/` 對 4 個 subcommand 全部 byte-equal pass
- [ ] 4.11 Phase C manual smoke：拿真 `D:/side_project/uv` repo + 真 `claude` CLI，跑 `codebus init` + `codebus goal "..."`，肉眼檢查產出 vault 結構 + lint output；對比 legacy/ts-src/ 跑同樣 goal 的差異

## 5. Phase D — cleanup

- [ ] 5.1 Verify all conformance gates pass（Phase A、B、C 全綠）+ 152 個原 TS test（在 legacy/ts-src/ 上仍可跑）對齊 Rust test 數量 ≥ 80% coverage
- [ ] 5.2 Cool-down period：保留 legacy/ 一週、期間對 buddy-gacha 與 1 個公開 repo 跑 manual smoke，任何 user-facing behavior diff 翻 legacy 對照修 Rust
- [ ] 5.3 Remove legacy and TS toolchain：`git rm -r legacy/`、刪除 `package.json`、`tsconfig.json`、`node_modules/`、`vitest.config.*`
- [ ] 5.4 Update `.gitignore`：移除 TS 相關（`node_modules`、`dist/`），加 Rust 相關（`target/`、`Cargo.lock` 視 binary/lib 決定）
- [ ] 5.5 Update `CLAUDE.md` 反映 Rust-only 狀態：「Common commands」section 改為 cargo build / cargo test / cargo run；「Architecture」section 反映 3-crate workspace；删除 `npm run build` / `npm test` 指引
- [ ] 5.6 Final commit：「rust parity reached, removing ts reference impl after cool-down period」

## 6. Validate and archive

- [ ] 6.1 Delete parked change directory：`spectra` 不支援直接刪 parked、用 `rm -rf .git/spectra-app/changes/wiki-hygiene-signals` 移除（兩條 rule 已併入本 change wiki-lint spec delta）
- [ ] 6.2 Run `spectra validate rust-rewrite` — 全綠
- [ ] 6.3 Run `cargo test --workspace` + `cargo clippy --workspace -- -D warnings` + `cargo fmt --all -- --check` 全綠
- [ ] 6.4 Final acceptance review：proposal 列的 BREAKING change 全部完成、Non-Goals 沒被誤觸（Tauri 仍未實作、無新 lint rule、distribution 未動）
