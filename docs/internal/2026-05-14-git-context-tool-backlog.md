# Backlog: 加 git 指令給 agent + PII 覆蓋

**Date:** 2026-05-14
**Surfaced during:** v3-app-workspace-goal apply（user observation）
**Severity:** feature gap with PII safety implication
**Owner:** harry
**Status:** parked

---

## 觀察

目前 agent toolset：

```rust
// codebus-core/src/verb/goal.rs
pub const GOAL_TOOLSET: &[&str] = &["Read", "Glob", "Grep", "Write", "Edit"];
```

沒 Bash、沒 git。對寫 wiki 是 loss：

- `goal` 寫不出 lineage（"module X 是從 commit Y 演化來的"）
- `goal` 寫不出 rationale（"為什麼這樣 design 看 commit ABC 的 message"）
- `query` 在 wiki 過時時補不上 fresh 資訊
- `fix` 看不到 recent breakage 是哪 commit 引起

潛在 high-value 工具：`git log` / `git blame` / `git show` / `git diff` / `git rev-parse`（全 read-only 子集）。

## PII 阻力

`pii::scanners::regex_basic::RegexBasicScanner` 只在 `raw_sync` 階段掃 **source code 內容**（copy from `<repo>/<src>/` → `.codebus/raw/code/`）。git layer 完全 bypass：

| 資料 | PII 掃過嗎 |
|---|---|
| source file 內容（mirror 進 `raw/code/`） | ✓ |
| `git log` author name / email | ✗ |
| `git log` commit message | ✗ |
| `git blame` per-line author | ✗ |
| `.codebus/.git/` 自己 wiki commit log | 半 ✗ |

實際 risk：

- author email = 個人 gmail / 公司 email → 洩漏進 wiki
- commit message 可能含 jira ticket、deploy log、debug 輸出含 secret
- 加裸 Bash + 白名單 git **=** 破壞 codebus 「PII-sanitized wiki」核心保證

## Proposed fix（三個遞進方案）

### (a) Inline PII filter on git output

最小侵入：把 git command output 餵進既有 `regex_basic` scanner 再 stream 給 agent。

- toolset 加 `Bash`
- bash_whitelist = `"^git (log|blame|show|diff|status|rev-parse|cat-file) "`
- 在 `agent::invoke` 端 hook：spawn child process 後 wrap stdout 進 scanner → mask 後再 emit 到 agent stream

問題：scanner 是 binary mask、不知道 git 結構（不能 "redact author email 但保留 commit hash"）。粗暴但 work。

### (b) 專屬 git context tool（推薦）

加 first-class IPC / MCP tool（不裝 Bash）：

- `git_log_sanitized(repo, opts)` → returns `Vec<CommitSummary { sha7, date, message_sanitized }>` (author email mask）
- `git_blame_sanitized(repo, file)` → returns `Vec<BlameLine { sha7, message_sanitized, line }>`
- `git_show_sanitized(repo, sha)` → returns `CommitDetail { sha7, message_sanitized, diff }`

實作 detail：

- 統一 sanitize step：author name → "anonymous"、author email → "redacted@example.com"
- commit message 過 PII scanner（既有 regex_basic）
- diff 內容 = source code → 已被 raw_sync 掃過（但 git diff 是直接從 .git，不是 raw mirror — 需要重掃）
- agent 端只看 sanitized struct，沒辦法 escape 拿 raw

cleaner、但要新增 IPC / tool 抽象。

### (c) Pre-mirror history into raw/

`raw_sync` 階段擴：除了 source code，也把 git history 一次性 export 進 `.codebus/raw/history/`：

- `raw/history/commits.jsonl`：每行一個 sanitized `CommitSummary`
- `raw/history/blame/<file>.jsonl`：每檔對應 sanitized blame

agent 用 `Read` / `Grep` 既有 toolset 即可（不需要新 tool），跟 source code 統一 path。

問題：

- raw_sync 變更慢（要跑 `git log --all` + per-file `git blame`）
- 新增 source-signal dimension（git head sha） → 影響 drift detection
- disk overhead（大型 repo blame 很多檔）

## Tasks（粗估，若採 (b) 方案）

1. spec MODIFIED `pii-filter`：加 「git metadata sanitization」requirement
2. spec ADDED `git-context-tool`：定義 `git_log_sanitized` / `git_blame_sanitized` / `git_show_sanitized` 接口
3. `codebus-core/src/git/sanitize.rs`：sanitize helper（author redact + message scan）
4. `codebus-core/src/git/context.rs`：3 個函數實作
5. `codebus-core/src/agent/`：把 git context 透過 MCP tool 或 child process command channel 暴露給 agent
6. integration test：sanitize round-trip、agent invoke 時拿不到 raw author email

工程量：~2-3 個半天（方案 b）；方案 a 約 1 天但 PII 保證較弱。

## Out of scope

- 加裸 Bash + 任意 shell（永久禁止——攻擊面太大）
- write-side git commands（`git commit` / `git push`）—— agent 不該寫 git
- 修改 `.codebus/.git/` 內部 wiki commit author（既有 auto-commit 行為不動）

## 何時動

優先序低於 v3-skill-bundles-vault-only（那條更輕、更獨立）。

可考慮在 D `v3-app-chat-cmdk` 之後做——chat verb 也會受益（chat 想看 history 一樣需要 git tool）。

## 替代：什麼都不做

目前沒這工具 wiki 仍可寫得不錯（基於 source code）。是 nice-to-have，不是 critical。
