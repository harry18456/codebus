## Problem

User 在 vault A 跑 goal、回 Lobby、進 vault B、起新 goal 被 backend 拒絕 `AppError::Invalid { field: "active_runs", message: "another goal run is already active" }`。從 user 角度看是 regression / UX 破裂：在 vault A 的工作影響到 vault B 開新工作的能力。

## Root Cause

Spec 與實作直接矛盾。

**Spec**（`openspec/specs/app-workspace/spec.md` `Requirement: One Active Goal Run At A Time`、line 911 + 918）：

> "at most one goal-mode `run_goal` invocation is active **per vault** per app session"
> "This invariant applies per app session within a single vault; switching vaults (back to lobby then opening a different vault) **does not carry the constraint across**."

**實作**（`codebus-app/src-tauri/src/state/active_runs.rs`）：

`ActiveRuns` 是單一 process-wide `HashMap<String, Arc<AtomicBool>>`、key 只有 `RunId`、**完全不含 vault 資訊**。`has_goal_run()`（line 70-73）對全 map 做 `keys().any(|k| !k.starts_with("chat-"))`、不分 vault 就會 hit。

`spawn_goal`（`codebus-app/src-tauri/src/ipc/goals.rs:217`）呼叫 `active_runs.has_goal_run()` 不傳 vault → 任何 vault 有 active goal 就擋掉所有 vault 的 spawn → **跟 spec line 918 直接衝突**。

Hypothesis a'（spec/impl gap）為唯一動工方向、static 驗證已成立、無需先 dynamic reproduce。Hypothesis b'/c'/d'（orphan entry / thread 卡死 / frontend race）defer：a' 修完若 user reproduce 仍失敗再追、大機率消失。

Frontend `useGoalsStore.reset()` 於 `codebus-app/src/components/workspace/Workspace.tsx:122-125` Workspace unmount cleanup 已正確呼叫、frontend 不是 bug source。

## Proposed Solution

**主軸**：補 `ActiveRuns` per-vault scope、讓 impl 對齊 spec line 918。

A. `ActiveRuns` key 從 `String` 改 `(VaultKey, String)` tuple（`VaultKey` = `String`、用 `Vault::path()` canonical 表達；之後若 `vault_id` landed 可改 typed）
   - `insert(vault, run_id, cancel)` / `remove(vault, run_id)` / `get(vault, run_id)` 加 vault param
   - 新增 `has_goal_run_for_vault(&str) -> bool`、廢除既有 `has_goal_run() -> bool`（grep 確認 7 production consumer 全 migrate）
   - `has_chat_turn` / `has_quiz_run` 同步加 vault-scoped 版本、舊版本廢除（per exhaustive sweep；spec wording 後述）

B. IPC layer migrate
   - `spawn_goal` / `cancel_goal_impl`（`codebus-app/src-tauri/src/ipc/goals.rs`）改傳 `vault_path` 到 `active_runs` 操作
   - `spawn_chat_turn` / `cancel_chat_turn`（`codebus-app/src-tauri/src/ipc/chats.rs`）同步加 vault scope
   - `spawn_quiz_plan` / `spawn_quiz_generate` / cancel（`codebus-app/src-tauri/src/ipc/quiz.rs`）同步加 vault scope

C. Spec scenario 補
   - `app-workspace` `One Active Goal Run At A Time` 加新 scenario「Cross-vault goal spawn allowed」
   - 既有 4 scenario 數值不動、僅補新 scenario 表達 cross-vault 行為

D. Spec wording 對齊（chat / quiz）
   - `app-workspace` chat-spawn 段（line 1306 附近）spec 沒明寫 per-vault；本 change 把 `has_chat_turn` 改 per-vault 是「spec silent → 沿同邏輯處理」的 conservative 決定、design.md 紀錄設計意圖；spec 不加新 wording、避免擴張 spec scope
   - quiz spec 同樣沒寫 per-vault、同邏輯處理

E. Regression test
   - `active_runs.rs` unit test 加 per-vault cases（同 vault 同 mode 拒絕 / 跨 vault 同 mode 允許 / chat × goal × quiz 三 mode 跨 vault 互不干擾）
   - `goals.rs` integration test 加 cross-vault concurrent goals 場景

## Non-Goals

- 不動 spec line 911 "per vault per app session" 規範本體（既有寫法已支持 cross-vault、改寫等於改 design）
- 不擴張 spec 把 chat / quiz 也明寫 per-vault（spec silent 留給未來 change、本 change 只動 impl 對齊 goal spec）
- 不為 hypothesis b'（orphan on spawn fail）/ c'（thread 卡死）/ d'（frontend race）寫 defensive 補丁；a' 修完 user reproduce 若仍失敗、再開獨立 change 追
- 不加 UX cross-route active-goal indicator（spec 允許 cross-vault、indicator 不再需要）
- 不重構 `ActiveRuns` 為 lock-free map / sharded mutex（per 既有 doc comment「contention is effectively zero」、premature optimization）
- 不動 frontend `useGoalsStore` lifecycle（Workspace.tsx 已正確 cleanup、frontend 非 bug source）

## Success Criteria

1. CDP smoke 跑 sequence（add vault A → spawn goal in A → wait completion → back to Lobby → add vault B → spawn goal in B）三次連跑、每次 vault B spawn_goal 都成功（不 reject）
2. CDP smoke 再加邊界：vault A goal **仍在跑中** → 同時 lobby → 開 vault B → 在 B spawn goal **應成功**（spec line 918 cross-vault allowed 場景）
3. Backend `active_runs` query：vault A goal 完成後 active_runs 對 vault A 為 empty、vault B spawn 不被 vault A 殘留影響
4. `pnpm tsc` + `pnpm test` + `cargo test`（含新加的 per-vault regression test）全綠
5. Spec `openspec/specs/app-workspace/spec.md` `One Active Goal Run At A Time` 新增「Cross-vault goal spawn allowed」scenario、`spectra validate vault-switch-goal-regression` 綠
6. Grep `has_goal_run\b`（不含 `_for_vault` 後綴）於 `codebus-app/src-tauri/src/` = 0 hit（舊版完全廢除）
7. Grep `has_chat_turn\b` + `has_quiz_run\b`（不含 `_for_vault` 後綴）於同路徑 = 0 hit（同步廢除）
8. 既有 spec scenario `Second spawn_goal during active run rejected at backend` / `Spawn allowed after cancel completes` / `Chat turn does not block concurrent goal spawn` 對應 unit test 仍綠（per-vault scope 不破壞同 vault 同 mode 互斥）

## Impact

- Affected specs: `app-workspace`（`One Active Goal Run At A Time` 加新 scenario）
- Affected code:
  - Modified:
    - codebus-app/src-tauri/src/state/active_runs.rs
    - codebus-app/src-tauri/src/ipc/goals.rs
    - codebus-app/src-tauri/src/ipc/chats.rs
    - codebus-app/src-tauri/src/ipc/quiz.rs
  - Test files updated（同 file 內 #[cfg(test)] mod 或 src-tauri/tests/）:
    - codebus-app/src-tauri/src/state/active_runs.rs（內嵌 unit test mod）
    - codebus-app/src-tauri/src/ipc/goals.rs（內嵌 #[cfg(test)] tests）
- Affected behavior（user-observable）:
  - vault A spawn goal、A goal 完成 / 仍在跑 → vault B 在 A 跑完前後均可 spawn goal、不再被擋
  - 同 vault 同 mode（goal × goal）互斥行為**完全不變**（既有 4 scenario 還是 true）
  - chat 同樣 per-vault scope、跨 vault 允許多 chat（spec silent、設計意圖 alignment）
  - quiz 同樣 per-vault scope（spec silent、同設計意圖）
- 跨檔同名詞 disambiguation 風險：「active goal」三層意義必須清楚（frontend `activeRun` 是 store field、backend `active_runs` 是 process state map、spec 「Active Goal Run」是邏輯不變量）；apply 階段 Task 1.1 disambiguation 步驟必走
