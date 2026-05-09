## Context

`codebus-cli/src/commands/query.rs` 是 7-line stub。v3-goal #5 已 ship `agent::claude_cli::invoke`、SKILL.md workflow content 機制、mock-claude integration test 框架。Query 是 read-only 對應 — reuse 大量 #5 基建，差別在 toolset / vault-precondition / 無 auto_commit。前面 conversation discuss 已對齊 5 條 assumptions（reuse invoke + 4-step flow + strict refuse + read-only dual-layer + spec delta 兩 capability MODIFIED）。

## Goals / Non-Goals

**Goals:**

- `codebus query "..."` end-to-end：spawn agent → 印 answer → exit
- agent 在 vault cwd 跑、toolset 鎖死 `Read,Glob,Grep`、vault 內任何檔不被改
- vault 不存在時 strict refuse + 提示先 init（不 fall-back auto-init）
- SKILL.md 顯式 read-only invariant 重申（dual-layer：binary `--tools` + SKILL.md 文字）
- query 結果語言跟 query 文字一致（reuse vault `CLAUDE.md` §0 Language Policy）
- integration test 覆蓋 4 個 spec scenario（read-only sandbox / vault-missing refuse / no auto_commit / exit code propagate）

**Non-Goals:**

- `agent::claude_cli::invoke` 抽象成 trait（撞 [feedback_dont_speculative_abstract](file:///C:/Users/harry/.claude/projects/D--side-project-codebus/memory/feedback_dont_speculative_abstract.md)，等 #8 fix 也 ship 後 3 caller 再評估）
- query 自動 detect source drift 提示 user：read-only verb 不該觸發 raw_sync 副作用
- query 結果 caching / 持久化 / nested git commit：純 stdout 一次性
- 跟 #5 共抽 helper：query 4-step 跟 goal 5-step 重疊段只「resolve + invoke + propagate」3 段；提取 helper 後仍要 if-else 分支判 auto-init / detection / auto_commit，省不到 LOC

## Decisions

### Reuse `agent::claude_cli::invoke` + read-only toolset

```rust
const QUERY_TOOLSET: &[&str] = &["Read", "Glob", "Grep"];

invoke(InvokeAgentOptions {
    slash_command: format!("/codebus-query \"{}\"", args.text),
    vault_root: paths.root.clone(),
    toolset: QUERY_TOOLSET,
})
```

**Rationale**：

- `agent::claude_cli::invoke` 已 spike-verified（2026-05-09 5-cell `_pii-toolgate-spike`）：`--tools` 是真 hard gate，未列名工具完全擋 — toolset = `Read,Glob,Grep` 物理上排除 Write/Edit/Bash
- v3-goal ship 時 `InvokeAgentOptions::toolset` 設計成 `&'static [&'static str]`，goal 傳 5 元素 slice、query 傳 3 元素 slice，無需改 agent 模組
- 三旗（`--tools` / `--allowedTools` / `--permission-mode acceptEdits`）跟 cwd / stdio 行為跟 goal 完全相同

### Binary 4-step 流程（vs goal 5-step）

```
codebus query "..."
  ↓
1. clap parse: query text + (inherit) --repo + --debug
2. resolve repo → vault_paths(repo).root
3. precondition: paths.root 不存在 → eprintln + exit 2
   ✗ NO auto-init fallback (query 是 wiki user, 不 trigger ingest 副作用)
   ✗ NO source-signal detection (read-only 不需 source 最新)
4. spawn agent (toolset Read/Glob/Grep) + stdio inherit
5. propagate child exit code
   ✗ NO auto_commit (read-only 不 mutate vault state)
```

**Rationale**：

- query 是 wiki **user** 不是 ingest **producer** — init / goal 才 mutate vault；query 期待 vault 已存在且有 wiki 內容（即 user 至少跑過一次 goal）
- vault-missing strict refuse 的 ergonomic cost 是 user 多敲一次 `codebus init`，換來 mental model 清楚
- query 結束 `auto_commit` 在 clean tree 上是 no-op、dirty tree 才 commit；read-only invariant 下 working tree 應該永遠 clean，呼叫 auto_commit 是「冗餘且 misleading」 — 不如顯式不呼叫

**Alternatives considered:**

- (A) auto-init fallback 跟 goal 對稱：但 init 後 vault wiki 為空、query 對空 vault 沒結果，auto-init 沒實質幫助
- (B) detection 觸發 stderr warning 不 re-sync：增加 binary 複雜度且對 user 沒明確指示要做什麼
- (C) 無條件 call auto_commit：若 agent 違反 read-only invariant 寫了檔 → silent commit「wiki: {query}」反而 mask bug；不 commit 讓 dirty tree leak 出來才容易抓到 invariant violation

### Vault-missing strict refuse: exit 2 vs exit 1

`exit 2` 對應 v3-init 既有的 `sanity_check::check_repo_is_not_vault` 錯誤 exit code（其他 fatal error 如 spawn fail / auto_commit fail 用 exit 1）。Query 的「missing vault」歸類為 **user input error**（user 對沒 init 過的 repo 跑 query 是 user 操作問題），跟 sanity check 的「target 已是 vault」性質一致。

**Alternatives considered:**

- exit 1：可，但 exit 2 跟 sanity-check 對齊更一致（user-facing 錯誤都 exit 2）
- exit 0 + stderr message：silent fail，違反 「shell convention：non-zero exit 表 user-action-needed」

### Read-only invariant 雙層保護

| Layer | 機制 | 強度 |
|---|---|---|
| Binary `--tools "Read,Glob,Grep"` | toolset hard gate（spike-verified） | system-level |
| SKILL.md 內 「You MUST NOT use Write or Edit」 + binary 已 gate 說明 | agent instruction 重申 | best-effort |

雙層的價值：即使未來 Claude Code spec 變動 `--tools` 機制，SKILL.md 內的顯式 invariant 仍會降低 agent 違反 read-only 的機率。Defense-in-depth。

### SKILL.md content：4-step 跟 v3-goal 的 abstract / English / no-literal-template lessons 對齊

- Step 1：解析 query intent + 預測 relevant taxonomy types
- Step 2：glob `wiki/`、read 候選 page 的 frontmatter 過濾、命中後再 read body
- Step 3：跟著 `[[wikilink]]` 跨 page 但 bound depth 防 drift
- Step 4：emit answer 到 stdout（abstract instruction）— 語言跟 query 一致per cwd `CLAUDE.md` §0、不 copy literal verbatim

明確含「read-only invariant」段落 — 「MUST NOT use Write or Edit」+ 「toolset 已在 binary 層 gate」說明。

## Risks / Trade-offs

- [Risk] User 期待 query 對 fresh repo 也 work（auto-init）→ Mitigation：strict refuse + stderr message 明寫「先跑 codebus init」， user 一次學習成本後養成正確 mental model
- [Risk] agent 違反 read-only invariant 試圖 Write → 實機由 binary `--tools` hard gate 直接擋；但 agent 對「為什麼 Write 失敗」可能困惑 → Mitigation：SKILL.md 顯式宣告「toolset gated at binary layer」，agent 看到 instruction 就知道為何拒絕
- [Trade-off] query 4-step 跟 goal 5-step 共用 invoke 但不 dedupe binary flow → 等 #8 fix 也 ship 後再 refactor，維持當下「single-impl 不 trait 化」原則
- [Trade-off] vault-missing exit 2 對 shell script 寫的 wrapper 是新 exit code → 接受，v3 0.3.0-dev 階段無外部 wrapper

## Migration Plan

不適用 — query 之前是 stub。User 從 stub-fail 升級到 strict-refuse-missing-vault，stderr message 引導下一步。

## Open Questions

無待解 — discuss 階段已對齊全部 5 條 assumption + 1 個 strict-refuse open question 用戶選 A。Implementation 細節（具體 task split / SKILL.md 字面）由 tasks.md 與 implementation 處理。
