## Context

`lint_wiki()`（`codebus-core/src/wiki/lint/mod.rs`）目前是 pure-read 診斷：跑完 7 條 rules 報出 issues 後就結束。`--goal` 在 ingest 尾端呼叫它，但只把結果放進 `RunGoalResult.lint`、stderr 渲染後不做任何修正動作。`--check` 命令把同一個 lint 當 CI gate（exit code 0/1）。

剛 archive 的 plugin-architecture-refactor 已 ship `LlmProvider` trait（`codebus-core/src/llm/provider.rs`）+ `lint_wiki()` 與 7 條 rules（`codebus-core/src/wiki/lint/rules/`）。骨架完整，缺的是「lint → 餵 LLM 修 → 再 lint」這個循環，以及讓既有 vault（含手寫的 Obsidian vault）能不跑 goal 直接整理的入口。

策略上，這也是 `LlmProvider` trait 的**第一個多回合 caller pattern** — 目前只有 `goal` 的單回合 ingest。讓 fix loop 用「假記憶」（git diff 塞 prompt）撐多回合，正好能壓出 trait 在多回合場景的真實需求，為下一階段 #4 multi-LLM 該不該擴 trait（加 `session_id` / `with_history`）提供 grounding。

## Goals / Non-Goals

**Goals:**

- 提供 `lint_and_fix(vault_root, provider, max_iterations) -> FixReport` 函數作為單一事實來源
- `--goal` 流程在 `lint_wiki()` 之後自動接 fix loop，預設開
- 新增 `codebus --fix` 獨立命令，直接對既有 vault 跑 fix loop（不做 ingest、不寫新內容）
- 0 issue 直接短路、不浪費 LLM 呼叫
- 所有 7 條 lint rules 都丟 LLM 處理（包含 duplicate_slug、unexpected_file，這兩條看似可 deterministic 實則需要語義判斷）
- 終止條件兩條：`issue_count == 0` 或 `iter == max_iterations`（無 oscillation guard）
- Escape hatches 完整：CLI flag `--no-fix` / `--fix-max-iter N`、config `lint.auto_fix.{enabled, max_iterations}`
- `--check` 命令完全不變（保持純讀、不呼叫 LLM、CI gate 可信賴）

**Non-Goals:**

- 不擴 `LlmProvider` trait — 多回合靠 prompt 假記憶撐，trait 是否該加 `invoke_continued()` / `with_history` 留到 #4 階段
- 不改 lint rules 本身 — lint 模組保持 pure-read 契約
- 不裝 oscillation guard — 振盪靠 `max_iterations` 上限收斂
- 不持久化 fix 過程 — 不寫 RunLog / jsonl，stderr 即時顯示就好
- 不解 stream events 當記憶 — 統一以 `git diff wiki/` 當客觀事實
- 不對 `--query` / `--check` 加 fix loop
- fix loop 不對 wiki 內容做 PII 過濾（PII 只作用於 raw mirror，沿用既有 scope）

## Decisions

### Fix loop 模組位置：`codebus-core/src/wiki/fix/`

**選擇**：新增 `codebus-core/src/wiki/fix/` 模組（與 `wiki/lint/` 並列，不嵌套），三個檔：

```
codebus-core/src/wiki/fix/
├─ mod.rs       # lint_and_fix() 入口 + FixReport / FixError
├─ prompt.rs    # build_fix_prompt(issues, prior_diff) + per-rule fix hints
└─ memory.rs    # git_diff_summary(vault_root, since_commit) -> String
```

**為什麼**：
- 與 `lint/` 並列而非嵌套：fix 消費 lint 的 output，但本身會「呼叫 LLM 並寫檔」，跟 lint 的 pure-read 契約是兩個不同的 invariant。同一個模組底下混兩種 invariant 違反 plugin-architecture-refactor 建立的「每個 module 一個 invariant」原則
- 三個檔分工清楚：`mod.rs` orchestration + 公開 API、`prompt.rs` 純 prompt 構造（無 IO）、`memory.rs` 純 git shell-out（無 LLM）

**Alternatives considered**：

- **塞進 `wiki/lint/` 底下**：直觀但會破壞 lint「pure read」契約，未來 lint rules 會分不清能不能寫
- **獨立 crate `codebus-fix`**：scope 太小、不值；fix loop 必依賴 `wiki/lint` + `llm` + `git`，留在 `codebus-core` 共用既有依賴更省

### 「上一輪做了什麼」記憶來源：`git diff wiki/`

**選擇**：每輪結束、進下一輪前，跑 `git diff <prev_commit> -- wiki/` 取得結構化變動摘要，塞進下一輪 prompt 的 `<previous_attempt>` block。

**為什麼**：
- **客觀事實 vs agent 自述**：stream events 中 agent 自述「我把 X 拆成 Y、Z」未必對應實際檔案異動；diff 是 fs ground truth，不會說謊
- **既有基建**：vault 已是 nested git repo（`.codebus/.git/`，`auto_commit` 在 `goal.rs:117` 已用），diff 命令可以直接執行；不用引新依賴
- **天然摘要結構**：diff 的 `+` / `-` 行 + 檔案 path 已經是給 LLM 讀的好格式，不用額外整理
- 上一輪 commit 的 SHA 在 fix loop 開始時 snapshot 一次就好（透過 `git rev-parse HEAD` against `<vault>/.codebus/`），之後每 iter 對照同一個 base

**Alternatives considered**：

- **解 stream events**：agent 自述未必是真做的、雜訊多、需要解析 tool_use / tool_result；複雜且不可靠
- **兩個都收**：diff 當事實 + stream 當意圖（雙保險）；意圖層的價值不夠大、開發成本翻倍
- **完全 stateless（不裝記憶）**：trivial 但 agent 會繞同樣的路試 5 次，浪費 token；假記憶這層投資是必要的

### 終止條件：兩條，相信循環

**選擇**：

```rust
loop {
    let issues = lint_wiki(vault_root).issues;
    if issues.is_empty() { break Ok(FixReport::Clean); }       // 條件 (a)
    if iter >= max_iterations { break Ok(FixReport::MaxIter); } // 條件 (b)
    apply_fix_iteration(...);
    iter += 1;
}
```

**為什麼**：
- **(a) issue == 0 收工**：天然成功條件
- **(b) iter == max（預設 5）強制收**：硬上限，防無限燒 token
- **不裝 oscillation guard**：原本想「issue_count 不降就停」當第三條，但會誤殺：第一輪修好兩個但戳出新的、第二輪「問題變多」實際上是進步。`max_iterations` 已經提供有界保護，再加啟發式只是過度設計

**Alternatives considered**：

- **加 oscillation guard**（兩輪 issue set 完全相同就停）：保守、但跟 max_iter 大致重疊；放棄
- **時間上限**（n 秒沒進展就停）：跨環境不穩、Windows / Linux 表現不一；max_iter 比較乾淨

### 全部 7 條 rules 都走 LLM

**選擇**：fix loop 不分 「agent-fixable / structural-only」，所有 lint issues 一視同仁餵 LLM。

| Rule | 為什麼丟 LLM |
|---|---|
| broken_wikilink | 要決定該移除 link 還是該補建那頁 |
| page_size | 要決定怎麼拆、拆成哪幾頁 |
| missing_nav | 要寫 index.md / overview.md 內容 |
| root_page | 要決定該歸到哪個 type folder |
| frontmatter_integrity | 要修 YAML 結構 + 補缺欄位 |
| **duplicate_slug** | 看似可 rename，但要決定保留哪個 / 要不要合併（語義） |
| **unexpected_file** | 看似可 mv 到對應 type folder，但歸到哪一類也是看內容（語義） |

**為什麼**：
- duplicate_slug / unexpected_file 的 deterministic 算法很難寫對：rename 規則 / type 歸類規則都需要看頁面內容。寫 deterministic 邏輯反而比 prompt 複雜
- 一致性：「wiki 是 agent 寫的、修也讓 agent 修」哲學上乾淨
- 全 LLM 也讓 prompt 結構單一（不用分 deterministic path 和 LLM path）

**Alternatives considered**：

- **deterministic 處理 duplicate_slug + unexpected_file**：兩條獨立 path，prompt 簡化但實作複雜化；放棄
- **rule-by-rule 開關**：每條 rule 一個 config 旗標決定走 deterministic / LLM；過度配置

### Prompt 結構：單一 batched，含上一輪 diff

**選擇**：每 iter 一次 invoke、prompt 含三段：

```
<lint_issues>
  <issue path="..." rule="..." severity="...">message</issue>
  ...
</lint_issues>

<previous_attempt iteration="N-1">
  <git_diff>
  ... (output of git diff ${prev_sha} -- wiki/)
  </git_diff>
</previous_attempt>

<task>
For each issue above, edit the corresponding wiki page or take the appropriate
structural action (rename, move, merge). Use Read/Edit/Write/MultiEdit tools.
After your changes, the linter will re-run; if there are remaining issues, you
will be asked to address them in the next iteration.
</task>
```

第一輪沒有 `<previous_attempt>` block。

**為什麼**：
- 一次 invoke 看到全部 issues：agent 修 A 時知道 B 也要顧，避免「修 A 戳出 B」的盲修；token 成本只比逐條低、且省去多次 invoke 的固定 overhead
- XML-ish tags：Claude Code agent 對 structured input 反應較好（既有 schema/CLAUDE.md 也用這種風格）
- prior diff 內嵌 prompt 讓 agent 看得到「上一輪我做了什麼」，避免重複嘗試同一條失敗路徑

**Alternatives considered**：

- **逐條 issue 一次 invoke**：N 條 issues = N 倍 invoke 成本；agent 也看不到全貌
- **priority-ordered top-N**：每輪只給最重要的幾條，慢慢收；複雜化終止條件、慢

### 0-issue 短路

**選擇**：`lint_and_fix` 開頭先跑一次 `lint_wiki()`，若 `issues.is_empty()` 直接 return `FixReport::Clean { iterations: 0 }`，不呼叫 LLM。

**為什麼**：
- 多數 goal 跑完 lint 是 clean 的（test fixtures、小 repo）；無條件 invoke 一次浪費 1 次成本
- 邊際成本低（一次 in-memory lint 而已）但省下的 LLM call 很值

**Alternatives considered**：

- **不短路**：實作簡單，但 90% case 浪費；放棄

### `--goal` 與 `--fix` 共用同一函數

**選擇**：

```rust
// codebus-cli/src/commands/goal.rs::run_goal
sync_repo_to_raw(...)?;
provider.invoke(goal_prompt).await?;     // 寫 wiki
enrich_source_metadata(...)?;
flag_stale_pages(...)?;
let lint = lint_wiki(...);
if !opts.fix_disabled {
    lint_and_fix(vault_root, provider, opts.max_iterations)?;  // ← 接這裡
}
auto_commit(...)?;

// codebus-cli/src/commands/fix.rs::run_fix
let lint = lint_wiki(vault_root);
lint_and_fix(vault_root, provider, opts.max_iterations)?;  // ← 直接這個
auto_commit(vault_root, "wiki: lint fix loop")?;
```

**為什麼**：
- 90% 邏輯共用，分兩個 change 才是浪費
- 兩個 entry point 差異只在「進來之前做了什麼」（goal 多 ingest + raw_sync），fix loop 自己對「vault 怎麼被改的」不關心，所以解耦

**Alternatives considered**：

- **`--fix` 之後另開 change**：分散注意力、共用邏輯被切兩半；放棄
- **共用 `commands/lint_and_fix_helper.rs`**：把 90% 邏輯抽到 helper，goal/fix 各自有 wrapper；過度抽象，直接呼 `wiki/fix/mod.rs::lint_and_fix` 就好

### Config schema：`lint.auto_fix` 子結構

**選擇**：

```rust
// codebus-core/src/config/schema.rs
pub struct LintConfig {
    pub disabled_rules: Vec<String>,
    pub custom_rules_dir: Option<String>,
    #[serde(default)]
    pub auto_fix: AutoFixConfig,
}

pub struct AutoFixConfig {
    pub enabled: bool,           // 預設 true
    pub max_iterations: u32,     // 預設 5
}

impl Default for AutoFixConfig {
    fn default() -> Self {
        Self { enabled: true, max_iterations: 5 }
    }
}
```

對應 yaml：

```yaml
lint:
  auto_fix:
    enabled: true
    max_iterations: 5
```

**為什麼**：
- 子結構而非平鋪：未來若要加 fix-specific 旗標（例如 `prompt_style`、`memory_strategy`）有地方擺
- `Default` 設 `enabled: true` 是 R 階段討論結論（agentic feel，預設開）

**Alternatives considered**：

- 平鋪 `lint.auto_fix_enabled` + `lint.auto_fix_max_iter`：擴展性差、命名雜
- `lint.fix.{...}` 縮寫：跟 spec scenario 描述「auto fix」對不上，不直觀

### CLI override：`--no-fix` + `--fix-max-iter N`

**選擇**：兩個 flag，覆蓋 config：

```
--no-fix              # 等價 lint.auto_fix.enabled = false
--fix-max-iter <N>    # 覆蓋 lint.auto_fix.max_iterations
```

`--no-fix` 與 `--fix-max-iter` 同時出現時，`--no-fix` 勝（fix 不跑、max_iter 無意義）。

**為什麼**：
- `--no-fix`：debug 時最常用「這次跑不要 fix」，CLI 比改 config 順
- `--fix-max-iter`：可調寬可調窄、debug 場景多

**Alternatives considered**：

- 只給 `--no-fix`，`max_iter` 必走 config：debug 場景太常用、值得快捷
- 加 `--fix` 強制開：跟「config 預設開」邏輯重複；放棄

## Risks / Trade-offs

- **Risk: 預設開導致每次 goal token 成本 2-3x（最壞 5x）**
  → Mitigation：(a) 0-issue 短路（多數情況不會跑）；(b) `--no-fix` 旗標讓使用者可單次關；(c) config `lint.auto_fix.enabled: false` 讓使用者永久關；(d) #3 token tracking 之後可顯示 fix loop 的 cost 佔比、讓使用者知道何時關才划算

- **Risk: 假記憶不夠用、agent 繞同一條路失敗 5 次都修不好**
  → Mitigation：(a) `max_iterations` 上限收斂、不會無限燒；(b) `FixReport::MaxIter { remaining_issues }` 回報剩下的問題、讓使用者知道哪些 fix loop 沒搞定；(c) 這個失敗本身就是 #4 multi-LLM trait 該不該擴的訊號 — 失敗率高就證明需要真記憶

- **Risk: 自動 git commit 把 fix loop 的中間態（修一半）也 commit 進 git history**
  → Mitigation：fix loop 整個跑完才呼叫 `auto_commit`（既有設計）；中間 iter 之間不 commit（但會被 git status --porcelain 看到 unstaged 變動，這是給下一輪 diff 取記憶用的，跑完再一次 commit）

- **Risk: `--fix` mode 對手寫 vault 跑時，lint 命中的可能是使用者刻意的設計（例如刻意留 broken wikilink 當 forward reference）**
  → Mitigation：(a) `--no-fix` 同樣作用於 `--fix` mode（變成等價 `--check`，本質上是 user error）；(b) 文件清楚說「`--fix` 會修改 wiki/」；(c) commit 訊息明確標 `wiki: lint fix loop`，使用者可 `git revert`

- **Trade-off: 全部 7 條 rules 都走 LLM、duplicate_slug / unexpected_file 沒做 deterministic 分支**
  → 接受：deterministic 邏輯本身需要語義判斷（duplicate 該保留誰、unexpected 該歸去哪），與其寫一個半吊子的 deterministic 算法，全交 LLM 反而簡單一致

- **Trade-off: trait 不擴、靠假記憶撐多回合**
  → 接受：這就是這個 change 的核心設計意圖之一 — 等假記憶碰壁再決定 trait 怎麼擴。預先擴 trait 可能設計錯方向（例如以為要 session_id，跑下來才知道要的是 explicit history）

## Migration Plan

單一 R 階段（scope 比 plugin refactor 小一個量級）：

1. 加 `codebus-core/src/wiki/fix/{mod.rs, prompt.rs, memory.rs}` + inline tests（0-issue 短路、max_iter 終止、git diff 摘要、prompt 結構）
2. `LintConfig` schema 加 `auto_fix: AutoFixConfig` 欄；loader 對應解析；既有 `~/.codebus/config.yaml` 沒設 `auto_fix` 走 default（enabled=true, max_iter=5）
3. `goal.rs::run_goal` 在 lint 之後接 `lint_and_fix()`；尊重 `RunGoalOptions.fix_disabled` + `max_iterations`
4. `main.rs` 加 `--fix` mode、`--no-fix`、`--fix-max-iter` 三個 CLI 參數；`run_fix_cmd` dispatcher
5. `commands/fix.rs` 新增 `run_fix(RunFixOptions)` — 對既有 vault 跑 lint_and_fix + auto_commit
6. `cargo test --workspace` + uv `--check` byte-equal（fix loop 不影響 --check 路徑）+ 新 `--fix` 對 buddy-gacha smoke
7. final commit：`feat(fix): lint feedback loop with --goal auto-fix and standalone --fix mode`

### Rollback 策略

單一 commit，rollback = `git revert <hash>`。`--check` 路徑不變、PII filter 不影響、既有 `--goal` 行為加 escape hatch（`--no-fix` / `lint.auto_fix.enabled: false`）即可恢復 0.2.0 行為。

### Cool-down

- 跑自己 buddy-gacha 一輪 `--goal "..."`（預設開 fix），觀察 stderr 看 fix iteration 數、token 成本、實際修了哪些 issue
- 跑一次 `--fix` 對既有 vault，確認獨立路徑會工作
- 跑一次 `--goal --no-fix`，確認 escape hatch 真的關掉 fix loop（行為等同 0.2.0）

## Open Questions

- **Per-rule fix hints 該嵌在 prompt 還是寫在系統 schema**：`prompt.rs` 對每條 rule 會給 LLM 提示「broken_wikilink 該怎麼修」之類。是放在 fix prompt 內 inline，還是擴 `codebus-core/src/schema/CLAUDE.md` 加一段「lint feedback loop 流程說明」？傾向 inline（per-iteration prompt 自包含、不污染 system prompt），但若 hint 太長 token 重複成本可觀，可能 R 階段重新評估
- **fix loop 失敗時 `RunGoalResult` 怎麼回報**：`FixReport::MaxIter { remaining_issues }` vs `FixReport::Clean { iterations }` 兩種終態，要不要影響 `goal` 的 ExitCode？傾向保守：fix loop 是 best-effort、不擋 commit；ExitCode 只反映 ingest 是否 OK（與 0.2.0 一致）。剩餘 issues stderr 顯示就好
- **`--fix` 對沒 vault 的 repo 該怎麼回應**：`<repo>/.codebus/` 不存在時，`--fix` 應該 (a) 報錯 + 提示 `codebus init` 還是 (b) 自動 init？傾向 (a) — `--fix` 預期作用於既有 vault，自動 init 會混淆 init / fix 的職責
