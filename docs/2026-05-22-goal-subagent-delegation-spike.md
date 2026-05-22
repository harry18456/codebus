# T5 Spike：goal 動態 subagent 委派（Task 工具）

**Date:** 2026-05-22
**Task:** loop T5（只讀探勘）
**背景:** [goal-subagent-delegation backlog](2026-05-21-goal-subagent-delegation-backlog.md)（2026-05-21，open/deferred）

---

## TL;DR

2026-05-21 backlog 的 grounding 對現碼核對**全屬實**，且它本就是「先記錄、低優先、deferral」項。本 spike 無法推進其關鍵阻塞（general-purpose subagent 能否寫檔需**真實 claude run** 的 ground-truth，loop 只讀做不到）。**唯一新增價值是一個 codex 跨 provider 缺口**：整套 `Task` + `--tools` 天花板機制是 **claude-only**，codex 路徑完全不同且 `subagent-sandbox-control` 的安全驗證**不涵蓋 codex**——backlog（2026-05-21）寫時尚未把 codex 納入考量。

---

## Grounding 核對（✅ 屬實）

- `GOAL_TOOLSET = ["Read","Glob","Grep","Write","Edit"]`（`verb/goal.rs:58`）——parent **含 Write/Edit、不含 Task**，正是 backlog「關鍵坑」的前提（裸給 Task → AI 可能 fallback general-purpose → 繼承 parent Write/Edit）。
- `GOAL_VERIFY_TOOLSET = ["Read","Glob","Grep"]`（`:66`，唯讀）。
- 全 core **無任何 `Task` 引用**（grep 零命中）→ 確認目前無 subagent 能力。
- **無 `.claude/agents/` ship**（find 零命中）→ 確認 researcher def 尚不存在，backlog 的 (B) 是 net-new。
- spawn cwd = vault `.codebus/`（`skill_bundle/mod.rs:36` 等）→ 確認 researcher def 該 ship 到 `.codebus/.claude/agents/`，授權路徑與 skill bundle 一致、不污染 repo root。

## 關鍵阻塞（loop 無法推進）

backlog 的動工前置是「ground-truth 測：goal toolset 下 general-purpose subagent 能否實際寫檔」。這需要**真實跑一次 claude goal + 觀察檔案系統**——只讀 loop 做不到，必須留給 harry 手動實測。本 spike 不改變這個結論。

## ⚠️ 新增缺口：codex 讓這條變成 provider-specific

backlog 假設 claude 的機制（`Task` 工具 + `--tools` 天花板 + `--strict-mcp-config`）。但 codex 已是第二 provider，整套不適用：

1. **`GOAL_TOOLSET` / `--tools` 對 codex 無效**：`SpawnSpec` 不帶 toolset 欄位，codex_backend 用 `-s sandbox` 把關、忽略工具白名單（PE1 確認）。所以「把 Task 加進 `GOAL_TOOLSET`」是**純 claude 動作**，對 codex 不生效。
2. **codex 有內建 `spawn_agent`**（多代理工具，見 [PE1 診斷] 引的 spike §219）。即在 codex 下 goal **可能已能開 subagent**，且不受 `--tools` 天花板約束——claude 的安全模型不轉移。
3. **`subagent-sandbox-control` 的「已決定不做/已驗證安全」結論是 claude-only**：它驗的是「claude `--tools` 正確排除 Task」。codex 的 `spawn_agent` 是**另一條未驗證路徑**，若要在 codex 下談 goal 委派，需對 codex 重做一次 sandbox/逃逸驗證。

→ 結論：這條若哪天要做，**不能再當成單一機制**——需拆 claude 路徑（加 Task 到 toolset + researcher def）與 codex 路徑（評估/限縮 `spawn_agent`、重驗安全）兩套。建議在 backlog 補這個 provider 維度。

## 建議

- 維持 backlog 的 **deferral**（無實測痛點、deletion test 通過、低成本可逆）——本 spike 無新證據改變優先序。
- **若哪天動工**：先 claude ground-truth 寫檔測（harry 手動）；同時把 codex 的 `spawn_agent` 行為與安全納入評估（新缺口）。
- 順手更新原 backlog：加註「機制 provider-specific，codex 路徑另計、安全需重驗」。

## 待 harry
此條本就低優先、等「想實驗 agentic 委派」訊號才動。若哪天要實驗，記得它現在是**兩 provider 兩套機制**，不只是「加一條 Task」。
