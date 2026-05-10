# v3 UV Repo Verification Report

**Date**: 2026-05-10
**Target**: `D:\side_project\uv` (uv repo, ~1289 files / 26 MiB)
**Binary**: `target/release/codebus.exe` (post v3-render-polish)
**Isolation**: `CODEBUS_HOME=/tmp/cb-verify-home`、`--no-obsidian-register`

## Banner Verification — All Pass

Subprocess runs are non-TTY → ASCII fallback (`▶`/`ok`/`~`/`!`/`.`/`✓`) per spec.

### `codebus init`
```
▶ 來囉來囉~ CodeBus 駛入 D:/side_project/uv...
ok 同步完成 (1289 檔, 26.0 MiB, 13564 ms)
! PII：regex_basic, scanned 1289, hits 672, action mask
. commit 652615a
✓ 掰掰~下車囉！wiki 已生成於 D:/side_project/uv/.codebus/wiki
```
5 banners (Start / SyncDone / PiiSummary / CommitDone / Done). No Hint banner (used `--no-obsidian-register`). ✅

### `codebus goal "Describe the top-level workspace structure..."`
```
▶ 來囉來囉~ CodeBus 駛入 D:/side_project/uv...
◎ 任務目標：Describe the top-level workspace structure...
~ 同步 source → raw/code...
ok 同步完成 (1289 檔, 26.0 MiB, 1585 ms)
Created 3 pages, modified 0 pages.       ← agent stdout passthrough
~ lint 中...
ok lint：0 errors, 0 warnings (1 ms)
. commit 4a5250c
✓ 掰掰~下車囉！wiki 已生成於 D:/side_project/uv/.codebus/wiki
```
8 banners + 1 line agent output. Output flow correct: codebus banner → agent stdio inherit → codebus banner. ✅

Generated content quality:
- `wiki/synthesis/workspace-overview.md` — listed all 67 uv member crates grouped by role
- `wiki/index.md` — `[[workspace-overview]]` entry with summary
- `wiki/log.md` — goal entry with suggested reading order

### `codebus query "What does the workspace-overview page say about uv-resolver?"`
```
▶ 來囉來囉~ CodeBus 駛入 D:/side_project/uv...
根據 workspace-overview 頁面的內容，關於 uv-resolver 的說明如下：
... (zh-tw answer)
```
1 Start banner only (query has no Done — by design, doesn't write wiki). 0 commits added (read-only invariant ✅).

### `codebus fix` — initial clean short-circuit
```
▶ 來囉來囉~ CodeBus 駛入 D:/side_project/uv...
~ lint 中...
ok lint：0 errors, 0 warnings (2 ms)
```
No agent spawn (precheck clean). No commit. ✅

### `codebus fix` — with planted broken wikilink
```
▶ 來囉來囉~ CodeBus 駛入 D:/side_project/uv...
~ lint 中...
**修復摘要：** 已移除 broken wikilink 並補錄至 index.md ...    ← agent fix summary
ok lint：0 errors, 0 warnings (75390 ms)
. commit 6e303a8
```
4 banners + agent fix summary. Agent edits verified:
- `concepts/broken-link-test.md`：`[[ghost-page-...]]` removed, replaced with explanation
- `index.md`：added `[[broken-link-test]]` Concepts entry
- final lint 0/0 ✅
- committed `wiki: lint fix loop` ✅

### `codebus lint` — text format post-tweak
```
# 2 pages + 2 nav files scanned, 0 error(s), 1 warning(s)

! wiki/concepts/broken-link-test.md
   warn:  broken wikilink in body: [[ghost-page-that-does-not-exist]] (no page named ...)
```
**`[rule_id]` suffix removed** per post-ship UX tweak. JSON format still has `rule` field for agents.

## Quality Findings

### 1. PII default `mask` 對 docs/test 過於激進

uv repo 觸發 672 PII matches，**多數是無害內容**：
- `127.0.0.1` (localhost) 在 `CONTRIBUTING.md`、test 中
- `example@... `email 在 test 的 `pyproject.toml` author 欄位
- Test fixture data in `tests/it/auth.rs`、`tests/it/edit.rs`

raw mirror 內容變成 `[REDACTED:ipv4]:8000` 之類，**降低 wiki agent 對源碼的可讀性**。

**建議**：
- 加 `pii.scanner: none` 給 docs-heavy / test-heavy repo
- 或考慮 v3-pii 的 severity 分流：`Critical` (AWS / Anthropic key) 走 mask；`Warn` (email / ipv4) 走 warn 不 mask
- 或加 `pii.patterns_exclude` 排除已知 false-positive 樣式

### 2. `init` 緊接 `goal` 觸發冗餘 re-sync

init 寫了 manifest source_signal；goal 第一步檢查 drift，本應 match → skip re-sync。但實際 goal stdout 顯示 `~ 同步 source → raw/code...` + `ok 同步完成 (1289 檔, 26.0 MiB, 1585 ms)` — 跑了 second sync。

可能原因：
- `walk_source_for_signal()`（goal 用）和 `sync_with_scanner()`（init 用）filter rule 不一致
- 或 `compute_source_signal()` 邏輯時序差異

需要進一步調查。**不影響 banner 行為**但浪費 1.6 秒 + 重新 emit 672 條 stderr PII warn。

### 3. `lint --repo <vault-root>` 靜默回 0 pages

`codebus lint --repo D:/side_project/uv/.codebus`（傳 `.codebus/` 自身）→ 回 `ok 0 pages + 0 nav files scanned, no issues`。應該：
- (a) error: `--repo expects source repo, not vault root` 或
- (b) 自動向上一層偵測 vault

正確用法是 `--repo D:/side_project/uv`（傳 source repo）。

### 4. Spawned agent 找不到 `codebus` binary on PATH

fix flow 跑時，agent 訊息：「**由於 `codebus` 指令在目前 shell 環境中不可用，無法執行官方 lint 驗證**」。

原因：
- 我用 `D:/side_project/codebus/target/release/codebus.exe`（絕對路徑）跑，未把 binary `cargo install` 到 PATH
- spawned claude 子程序繼承 PATH，但 PATH 沒 codebus → agent 無法 `Bash(codebus lint *)`
- CLI 最終 lint 仍 OK（parent 直接呼）→ trust-agent 模型未崩潰，但 agent **內部 iteration** 失能（只能 Read/Write/Edit 不能驗證）

**建議**：
- README quickstart 補一條 `cargo install --path codebus-cli` 或 `setx PATH ...`
- 或 init 寫 `.claude/settings.json` 時順便 inject 一條 `env.PATH` 加 codebus 安裝目錄
- 或 v3-fix-trust-agent spec 加一條 setup 前置條件 scenario

## 結論

✅ **核心功能 banner 全驗收通過**。default 模式 5-9 條 banner 串、ASCII fallback 正確、agent stdout passthrough 正確、commit / no-commit 行為符合 spec。

⚠ **4 條 quality findings** 都是 follow-up 等級，不阻 ship v3.0.0。優先順序我建議：#4（agent PATH）> #3（lint --repo UX）> #1（PII aggressiveness）> #2（redundant re-sync）。

vault 留在 `D:/side_project/uv/.codebus/`、isolated home 在 `/tmp/cb-verify-home`、log 在 `/tmp/cb-verify/*.{stdout,stderr}` 供你 inspect。
