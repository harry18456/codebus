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

> **Status (2026-05-10 同日更新)**：4 條 finding 全部 close。Critical floor 驗證通過、init→goal 不再 re-sync、`lint --repo <vault-root>` 兩種寫法輸出 IDENTICAL、README 補了 PATH 必要性段。詳見每條下方 **Resolution** 段與 commit hash。

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

**Resolution** — `v3-pii-severity-dispatch` (commit `c4f3d30`)：採第二建議。Critical 強 mask（不可關，security floor）、Warn default 從 `mask` 回 `warn`。`PiiSummary` banner 改 `action critical=mask, warn=<X>` 兩段式。User 仍可設 `on_hit: mask` opt-in 全 mask。實機驗證：UV repo `CONTRIBUTING.md` 內 `127.0.0.1` 保留原文；植入的 `AKIA...` key 即使在 default Warn 下仍被 mask。

### 2. `init` 緊接 `goal` 觸發冗餘 re-sync

init 寫了 manifest source_signal；goal 第一步檢查 drift，本應 match → skip re-sync。但實際 goal stdout 顯示 `~ 同步 source → raw/code...` + `ok 同步完成 (1289 檔, 26.0 MiB, 1585 ms)` — 跑了 second sync。

可能原因：
- `walk_source_for_signal()`（goal 用）和 `sync_with_scanner()`（init 用）filter rule 不一致
- 或 `compute_source_signal()` 邏輯時序差異

需要進一步調查。**不影響 banner 行為**但浪費 1.6 秒 + 重新 emit 672 條 stderr PII warn。

**Resolution** — `v3-bug-fixes` (commit `87e9b0c`)：實際 root cause 不是上面的猜測（`walk` vs `sync` filter 一致），而是 **`raw_sync` 的 `summary.bytes` 用 destination-side written bytes，而 `walk_source_for_signal` 用 source meta.len()**。Mask mode 下兩者必然不 match（672 個替換改變了 destination size）。修法：raw_sync 改用 `meta.len()` 累計 `summary.bytes`（source-side semantics），跟 walk 一致。實機驗證：`detect_drift=false`、goal stdout 不再印 SyncStart/SyncDone。

### 3. `lint --repo <vault-root>` 靜默回 0 pages

`codebus lint --repo D:/side_project/uv/.codebus`（傳 `.codebus/` 自身）→ 回 `ok 0 pages + 0 nav files scanned, no issues`。應該：
- (a) error: `--repo expects source repo, not vault root` 或
- (b) 自動向上一層偵測 vault

正確用法是 `--repo D:/side_project/uv`（傳 source repo）。

**Resolution** — `v3-bug-fixes` (commit `87e9b0c`)：採建議 (b) 自動向上一層偵測。`locate_vault_root` 對 `--repo` 路徑加 `wiki/` 子目錄偵測：含 `wiki/` 視為 vault root 直接用、否則 fall back 既有 `repo.join(".codebus")`。兼顧兩種寫法 + 路徑不存在的舊 contract。實機驗證：`lint --repo <source>` 與 `lint --repo <source>/.codebus` 兩種 stdout IDENTICAL。

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

**Resolution** — `docs(quickstart)` (commit `e4e8bfa`)：採第一建議。README 加 Quickstart 段含 `cargo install --path codebus-cli` 與獨立 "Why install on PATH" 段說明 fix loop sandbox 為何依賴 PATH 上的 codebus。沒採 init 自動 inject PATH（會放大 init 的副作用面，留給 follow-up 若有需要再開）。沒動 v3-fix-trust-agent spec（屬 setup 文件層而不是 spec 契約）。

## 結論

✅ **核心功能 banner 全驗收通過**。default 模式 5-9 條 banner 串、ASCII fallback 正確、agent stdout passthrough 正確、commit / no-commit 行為符合 spec。

✅ **4 條 quality findings 全部 close**（同日 commit）。v3.0.0 已 ship-ready。

vault 留在 `D:/side_project/uv/.codebus/`、isolated home 在 `/tmp/cb-verify-home`、log 在 `/tmp/cb-verify/*.{stdout,stderr}` 供你 inspect。

---

## 附錄: v3-run-log Manual e2e (Task 9.3)

執行於 2026-05-10，release build (`target/release/codebus.exe`)，target vault `D:/side_project/uv/.codebus`。

### (a) Stream events 即時可見
`codebus goal "name 2 source files in uv crate"` 執行期間 terminal 持續顯示：
- `→ [呼叫工具]` Glob/Read/Write 即時逐筆出現
- `← [觀察結果]` 200-char 截斷的 tool output
- `+ [正在生成]` Write 工具的特殊渲染
- `◆ [Agent 思考]` 模型中間 reasoning text

不再黑盒 — 與 v2 的 `claude-code-stream` 等價輸出。

### (b) RunLog jsonl 含完整 token usage
`D:/side_project/uv/.codebus/log/runs-2026-05-10.jsonl` 一條為例：

```json
{"goal":"name 2 source files in uv crate","mode":"goal","model":"opus","effort":"high",
 "started_at":"2026-05-10T05:25:59Z","finished_at":"2026-05-10T05:26:53Z",
 "tokens":{"input_tokens":11,"output_tokens":3642,"cache_read_tokens":153827,"cache_write_tokens":27110,
           "extras":{...full provider meta preserved...}},
 "wiki_changed":true,"lint_error_count":0,"lint_warn_count":0}
```

`extras` 完整保留 cache_creation breakdown / iterations / service_tier 等 provider-specific 欄位，未來換 model 不會丟資料。

### (c) `sink: none` 真的關掉持久化
`~/.codebus/config.yaml`：
```yaml
log:
  sink: none
```
重跑 `codebus goal` 後 `wc -l runs-2026-05-10.jsonl` 維持 3 條（未新增）。spec rename `Null` 變體成 `none` 的 YAML 字面值對齊 `pii.scanner: none` 的 foot-gun avoidance — 確認生效。

### (d) sink build 失敗 → warning + exit 0
`~/.codebus/config.yaml`：
```yaml
log:
  sink: jsonl
  dir: "D:/side_project/uv/.codebus/log/blocker.txt"   # 是檔案不是目錄
```
`codebus goal` 完整跑完，stderr 末段：
```
warning: run-log write failed (non-fatal): log sink io: 當檔案已存在時，無法建立該檔案。 (os error 183)
✓ 掰掰~下車囉！wiki 已生成於 ./.codebus/wiki
EXIT=0
```
即 `RunLog Write Failure Is Non-Fatal` 契約：log persistence 失敗 SHALL NOT 改變 verb exit code，僅 stderr 警告。

### Side findings (v3-run-log scope 內當場修)
- 初版實作意外帶上 `--input-format stream-json`：claude 把它解讀為「等 stdin streaming JSON」，與 `Stdio::null()` stdin 衝突→ child 立即退出 0、stdout 空、tokens 全 0。修：移除該 flag，input format 用 default `text` (prompt 由 `-p` 提供)。
- `wiki_changed_since_last_commit` 在新 vault (只有 1 commit) 跑 `git diff HEAD~1` 會洩 `fatal: bad revision 'HEAD~1'` 到使用者 terminal。修：`.stderr(Stdio::null())` 抑制；exit code 判斷邏輯不變。
