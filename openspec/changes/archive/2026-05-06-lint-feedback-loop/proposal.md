## Why

`--goal` 跑完 lint 報出問題（broken wikilinks、oversize pages、frontmatter 錯誤、duplicate slugs 等）後，**目前沒人修** — 只是停在 stderr，使用者得自己進去看。Wiki 是 agent 寫的，請 agent 自己改最自然；剛 archive 的 plugin-architecture-refactor 已 ship `LlmProvider` trait + `lint_wiki()` pure-read，骨架完整，缺的只是「lint → 餵 LLM 修 → 再 lint」這個循環，以及一個獨立 `--fix` 命令讓既有 vault（含手寫的 Obsidian vault）不用跑新 goal 也能整理。

這個 change 同時是策略上**第一個多回合 LLM 使用情境** — 目前 `LlmProvider` trait 只有 `goal` 一個單回合 caller pattern。讓 fix loop 用「假記憶」（git diff 塞 prompt）撐多回合，正好能壓出 trait 在多回合場景的真實需求 — 為 #4 multi-LLM 階段該不該擴 trait（加 session_id / explicit history）提供 grounding。

## What Changes

- 新增 `wiki/fix/` 模組，提供 `lint_and_fix(vault_root, provider, max_iterations) -> FixReport` 函數：
  - lint vault → 0 issue 直接短路、不呼叫 LLM
  - 否則 build batched prompt（含所有 issues + 上一輪 `git diff wiki/` 當「假記憶」）→ `provider.invoke(LlmMode::Ingest)` → re-lint
  - 終止條件：`issue_count == 0` 或 `iter == max_iterations`（無 oscillation guard，相信循環）
  - 全部 7 條 lint rules 都丟 LLM 處理（broken_wikilink、page_size、missing_nav、root_page、frontmatter_integrity、duplicate_slug、unexpected_file）— 後兩條看似可 deterministic，實則需要語義判斷（同名要選誰留、不在 5 種 type folder 的檔該歸去哪），全丟 LLM 反而乾淨
- `--goal` flow 在既有 `lint_wiki()` 之後接 `lint_and_fix()`，預設開；可透過 CLI flag `--no-fix` 或 config `lint.auto_fix.enabled: false` 關掉
- 新增 CLI mode `codebus --fix`：對既有 vault 直接跑 `lint_and_fix()`，不做 ingest、不寫新內容，純粹整理。共用同一個 fix loop 函數，只是 entry point 不同
- `--check` 命令**完全不變**（保持純讀、不呼叫 LLM、CI gate 可信賴）
- `LintConfig` schema 加 `auto_fix: { enabled: bool, max_iterations: u32 }` 兩欄；預設 `enabled: true`、`max_iterations: 5`
- CLI 加 `--no-fix` 與 `--fix-max-iter N` 兩個 override flag（debug / 一次性調整用）
- `LlmProvider` trait **不動**：保持 stateless single-turn；多回合靠「在 prompt 裡塞上一輪 diff」這個假記憶撐住

## Non-Goals (optional)

- **不擴 `LlmProvider` trait**：是否該加 `invoke_continued(session_id)` 或 `invoke_with_history(turns)` 留到 #4 multi-LLM 階段，等這個 change 跑下來看假記憶夠不夠用再決定
- **不改 lint rules 本身**：lint 模組保持「pure read」契約；fix loop 只是消費 lint 的 output
- **不裝 oscillation guard**：「修 A 戳出 B、修 B 戳出 A」的振盪情境靠 `max_iterations` 上限收斂，不額外做「issue_count 不降就停」這種啟發式
- **不持久化 fix 過程**：每次 fix loop 的 prompt / iteration 數 / 失敗 issues 不寫入 `RunLog` 或 jsonl；stderr 即時顯示就好（log 持久化是 #3 token tracking 階段的工作）
- **不解 stream events 當記憶**：stream 裡 agent 自述「我把 X 拆成 Y、Z」未必是真做的；統一以 `git diff wiki/` 當客觀事實，不解 stream
- **不掃 wiki/ 內容做 PII**：PII 過濾只作用於 raw mirror（既有設計，沿用）；fix loop 寫進 wiki 的內容不再過 PII
- **不對 `--query` / `--check` 加 fix loop**：query 是讀、check 是 CI gate，都該保持單一意圖

## Capabilities

### New Capabilities

- `lint-feedback-loop`: lint 結果驅動的多回合 LLM 修正循環。定義 `lint_and_fix` 的觸發路徑（goal 尾端 + 獨立 `--fix` mode）、終止條件（issue 清空 / max_iterations）、batched prompt 結構、`git diff wiki/` 當「上一輪做了什麼」的記憶來源、escape hatches（CLI flag + config）

### Modified Capabilities

- `wiki-ingest`: `--goal` flow 在 ingest + lint 完成後自動接 `lint_and_fix()`（預設開）；`--no-fix` flag 與 `lint.auto_fix.enabled: false` 配置可關閉

## Impact

- Affected specs:
  - New: openspec/specs/lint-feedback-loop/spec.md
  - Modified: openspec/specs/wiki-ingest/spec.md
- Affected code:
  - New: codebus-core/src/wiki/fix/mod.rs (lint_and_fix 入口 + FixReport struct)
  - New: codebus-core/src/wiki/fix/prompt.rs (build_fix_prompt + rule-specific fix hints)
  - New: codebus-core/src/wiki/fix/memory.rs (git_diff_summary — 取上一輪變動)
  - New: codebus-cli/src/commands/fix.rs (--fix mode handler)
  - Modified: codebus-cli/src/main.rs (新增 --fix mode、--no-fix flag、--fix-max-iter flag、wire run_fix_cmd)
  - Modified: codebus-cli/src/commands/goal.rs (run_goal 尾端接 lint_and_fix；尊重 fix_disabled flag + max_iter override)
  - Modified: codebus-core/src/config/schema.rs (LintConfig 加 auto_fix: AutoFixConfig 子結構)
  - Modified: codebus-core/src/config/loader.rs (parse lint.auto_fix.{enabled, max_iterations})
  - New tests: codebus-core/src/wiki/fix/ 內 inline tests 覆蓋 0-issue 短路、iteration 終止、git diff 記憶、終止條件三種 case；codebus-cli/tests/ 加 --fix mode integration test
ARTIFACT_EOF
echo "ok"
