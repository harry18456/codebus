## Context

`codebus-cli/src/commands/goal.rs` 目前是 7 行 stub。前面 thread（2026-05-08 ~ 2026-05-09）已對齊 v3-goal 整體 design：spawn pattern + auto-init fallback (v2 carry) + detection-based re-sync + triple-flag sandbox (verified by [`_pii-toolgate-spike` 5-cell](file:///D:/side_project/codebus/docs/v3-roadmap.md)) + spawn 收尾 `auto_commit` (reuse v3-vault-history 公開的 `codebus_core::git::auto_commit` API)。本 design.md 把那些 conversation-level 結論固化為實作可循的決策，並補實作層的細節（agent 模組形狀 / mock binary 注入 / commit on failure 行為 / SKILL.md 內容）。

## Goals / Non-Goals

**Goals:**

- `codebus goal "做 X"` 一條命令完成：vault precondition、source drift 偵測、agent invocation、變更 commit
- agent 在 cwd=vault root 跑，sandbox 鎖死 toolset = `Read,Glob,Grep,Write,Edit`，無法讀寫 vault 外部
- spawn 失敗（process spawn error）跟 agent exit non-zero 行為清楚分離
- integration test 不依賴真 `claude` binary（mock 注入避免 token 成本與 flaky network）
- `codebus-goal/SKILL.md` 從 placeholder workflow 段升級為 5-step ingest 內容

**Non-Goals:**

- `agent::claude_cli` 抽象成 trait（[anti-pattern #1](file:///D:/side_project/codebus/docs/v3-roadmap.md) — 寫 single-impl trait 是 speculative）
- query / fix 共享 invoke 接口的 generic 設計（#6 / #8 自己 wire 時再 refactor）
- 更動 init.rs 既有 raw_sync 邏輯（goal.rs 自己呼叫 `sync_with_scanner`）
- model / effort / token tracking / source enrich / stale detection（全留 #9 或 follow-up）

## Decisions

### Module 形狀：`codebus-core/src/agent/claude_cli.rs` single impl

```
codebus-core/src/agent/
├─ mod.rs                pub mod claude_cli; pub use claude_cli::*;
└─ claude_cli.rs         pub fn invoke(opts: InvokeAgentOptions) -> io::Result<ExitStatus>
                          pub struct InvokeAgentOptions {
                              pub slash_command: String,
                              pub vault_root: PathBuf,
                              pub toolset: &'static [&'static str],
                          }
```

**Rationale**：

- single impl 直接 `pub fn`，不寫 trait（無 second-impl 驗證；v2 archive 的 `LlmProvider` trait 跟 InvokeOptions struct 是反例，[v2 archive 內留下的 model/effort 字段](file:///D:/side_project/codebus/legacy/v2-rust/codebus-core/src/llm/provider.rs)註解就承認 leak Claude-specific 假設）
- `toolset` 用 `&'static [&'static str]` 而非 `Vec<String>`：caller 永遠傳 const slice（`&["Read", "Glob", "Grep", "Write", "Edit"]`），避免運行時分配
- 不接 `model` / `effort`：YAGNI；#9 v3-config 接 config 時再加 field

**Alternatives considered:**

- (A) trait + struct：v2 carry 但每次 trait 改動牽動 single caller，純 cost 無收益
- (B) factory enum：等 codex / gemini 真進來再開（[follow-up v3-multi-agentic-provider](file:///D:/side_project/codebus/docs/v3-roadmap.md)）

### Spawn args + stdio：三旗 + cwd + inherit

```rust
Command::new(claude_bin)
    .arg("-p").arg(opts.slash_command)
    .arg("--tools").arg(opts.toolset.join(","))
    .arg("--allowedTools").arg(opts.toolset.join(","))
    .arg("--permission-mode").arg("acceptEdits")
    .current_dir(&opts.vault_root)
    .stdin(Stdio::null())
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .status()
```

**Rationale**：

- `--tools` 真正 hard gate（spike E：`--allowedTools` 含 Write 但 `--tools` 不含時 Write 仍被擋）
- `--allowedTools` 同列表是 v2 iter-9 的冗餘保險網
- `--permission-mode acceptEdits` 是 -p mode 必加（spike A/B/C 證實沒下時 silent deny 全部 Write）
- `Stdio::inherit` 讓 user 即時看 agent 進度而非 buffer 到收尾才湧出
- `Stdio::null` for stdin 避免 -p 等待 stdin 那 3s warning（前面 nested claude spike 看到的訊息）

### Mock binary 注入：`CODEBUS_CLAUDE_BIN` env override

`agent::claude_cli::invoke` 內讀 env var `CODEBUS_CLAUDE_BIN`：

```rust
let claude_bin = std::env::var("CODEBUS_CLAUDE_BIN")
    .unwrap_or_else(|_| "claude".to_string());
```

Production: env 不設、`Command::new("claude")` 走 PATH。Test: env 指向 fake script（test fixture 內的 echo 子 binary），驗 args 構造對。

**Rationale**：integration test 必須 deterministic（不能依賴 user 系統有 claude / token quota / 網路）。env override 對 production 透明、test injection 乾淨。不開 trait 抽象就能達成 testability。

**Alternatives considered:**

- (A) trait + mock impl：撞 [anti-pattern #1](file:///D:/side_project/codebus/docs/v3-roadmap.md)
- (B) 真 spawn claude binary：每跑一次 test 燒 token、需要 auth 設定、CI flaky

### Source-signal detection：fail-safe 視為 drifted

```
detect(repo, manifest) -> bool needs_resync:
  current = compute_source_signal(repo)        // existing API
  match read_manifest(repo) {
    Ok(m) => current.git_head != m.source_signal.git_head
          || current.file_count != m.source_signal.file_count
          || current.total_bytes != m.source_signal.total_bytes,
    Err(_) => true,                            // fail-safe → re-sync
  }
```

**Rationale**：detection 失敗（manifest 損壞 / git 暫時不可讀 / I/O 錯）時，**re-sync 比 skip-sync 安全** — re-sync 最多只是重算一次 raw mirror（cost ~100ms）；skip-sync 可能讓 agent 看到過期 raw。Fail-safe 預設保守。

**Alternatives considered:**

- (A) detection 失敗即 error out：對 user 太 fragile（暫時的 I/O 錯誤就讓 goal 完全 fail）
- (B) 無 detection、永遠 re-sync：違反「source 沒變不重跑」優化的初衷

### `--force-resync` flag：唯一 escape hatch

flag 加進 goal subcommand 不加進 init / 其他 verb（query 是 read-only 不需要、fix 自己 lint loop 不該 mirror 重跑）。預設 detection-driven。`--no-resync` flag **不加**（YAGNI；user 想最快可以自己手動跑 init 後 goal）。

### Commit on failure：一律 commit（v2 carry）

不論 child exit code 是否 0，spawn 收尾都呼叫 `auto_commit(vault_root, "wiki: {goal}")`：

- child exit=0 + agent 寫 wiki page → dirty tree → commit 留 snapshot
- child exit=0 + agent refuse goal（out-of-scope）→ clean tree → auto_commit no-op、HEAD 不動
- child exit≠0 + agent 部分寫到一半 → dirty tree → commit「wiki: {goal}」留 partial 紀錄
- child exit≠0 + agent 完全沒寫 → clean tree → no-op
- codebus 用 child exit code propagate（auto_commit 失敗則 exit non-zero override）

**Rationale**：

- v2 carry 同行為（[v2 goal.rs:203](file:///D:/side_project/codebus/legacy/v2-rust/codebus-cli/src/commands/goal.rs) 沒 conditional）
- 「不論成敗都留 snapshot」對 user 看 nested git log 一致：「每次 goal trigger 都對應一個 commit-or-noop」，不需要心算「這次有沒有 commit」
- partial 紀錄是 vault diff 歷史的 feature，不是 bug

### SKILL.md 5-step workflow（goal verb only）

修改 `codebus-core/src/skill_bundle/mod.rs::stub_content` 函數，goal 分支回傳含完整 workflow 的字串；query / fix 分支維持既有 stub。新 workflow 內容：

```
## Workflow (per-goal ingest)

When this skill is activated, follow these 5 steps in order:

1. **探索 raw**：用 Glob / Read 掃 `raw/code/` 找跟 goal 相關的源碼。
   不需要把所有檔讀完整 — 抓 entry / module 級別的核心結構即可。

2. **規劃 page**：對照現有 `wiki/{concepts,entities,modules,processes,synthesis}/`，
   決定哪些 page 要新建、哪些要 update。每個 page 對應 schema 中的一個 page type。

3. **寫 frontmatter + body**：每個新 page 必須含 frontmatter（taxonomy / sources / 等）
   和 body content。Schema 規則細節讀 cwd 的 `CLAUDE.md`（不在這裡重複）。

4. **建立 wikilinks**：page 之間用 `[[other-page]]` 連結。連到既有 page 時用對方
   的 filename（不含路徑）。

5. **結束摘要**：印一行簡短的「本次新增 N 個 page、改了 M 個 page」摘要到 stdout，
   讓 binary 端的 user 看到結果。

完整 schema 規則（taxonomy 定義、frontmatter 格式、wikilinks 解析、stop criteria 等）見 cwd 的 `CLAUDE.md`。本 SKILL.md 不重複 inline schema rules。
```

**Rationale**：

- v2 [neutral.md §4](file:///D:/side_project/codebus/legacy/v2-rust/codebus-core/src/schema/neutral.md) 是同樣的 5-step 結構（v2 inline 進 schema、v3 移到 SKILL.md）
- schema rules 仍 by-reference 引用 cwd `CLAUDE.md`（[skill_bundle/mod.rs:73](file:///D:/side_project/codebus/codebus-core/src/skill_bundle/mod.rs) 既有）
- 不違反 [anti-pattern #2「Schema 不雙投遞」](file:///D:/side_project/codebus/docs/v3-roadmap.md)

## Risks / Trade-offs

- [Risk] integration test mock binary 跨平台寫法（Windows .bat vs Unix shebang）→ Mitigation：mock 用 Rust 寫一個 tiny test binary（cargo test 編譯時自動產出），跨平台 portable，不靠 shell script
- [Risk] Detection 對大 source repo（10000+ files）walk 一次成本可能 > 1s → Mitigation：v2 已實機驗證 walk-no-copy 比 walk-with-copy 快；spec 不限 perf threshold；如果未來真撞到再優化（cache last walk timestamp）
- [Trade-off] 一律 commit 失敗 case 會留「wiki: {goal}」commit 但 wiki 沒實際變動的 noise → 接受；user 看 git log 比看「沒留紀錄」資訊量大
- [Trade-off] `agent::claude_cli` 不寫 trait → 之後 #6 / #8 寫 query / fix 時會看到三份相似的 invoke 邏輯。可接受（duplication 比 speculative trait 安全；真要 dedupe 等三 verb 都 ship 後再開 refactor change）

## Migration Plan

不適用 — `goal` verb 之前是 stub，沒既存使用者。

## Open Questions

無待解 — 所有設計分歧（spawn / detection / commit-on-failure / mock / single impl）都在前面 thread 對齊，或於本 design 內敘明取捨。Implementation 細節（具體 task split / fake binary 怎麼寫）由 tasks.md 處理。
