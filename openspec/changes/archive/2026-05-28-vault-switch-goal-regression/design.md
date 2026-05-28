## Context

`codebus-app/src-tauri/src/state/active_runs.rs` 是 process-wide singleton（`AppRuntimeState.active_runs: ActiveRuns`），結構為 `Mutex<HashMap<RunId, Arc<AtomicBool>>>`。三個 mode 共用同 map、用 `RunId` prefix（無 prefix = goal、`chat-*` / `quiz-*`）區分。Pre-spawn guard 透過 `has_goal_run()` / `has_chat_turn()` / `has_quiz_run()` 查詢「全 process 是否有任一 mode 的 active run」。

`openspec/specs/app-workspace/spec.md` `Requirement: One Active Goal Run At A Time`（line 909-918）明寫「per vault per app session」+「switching vaults … does not carry the constraint across」。Impl 完全不分 vault 直接違反此 spec。

Frontend `useGoalsStore`（`codebus-app/src/store/goals.ts`）有 `activeRun` 單一 field（line 114）、`reset()` 在 Workspace unmount cleanup 中呼叫（`codebus-app/src/components/workspace/Workspace.tsx:122-125`）— frontend cleanup 路徑正確、bug 純粹在 backend impl 層。

### Pre-apply 校準（grep + Read 證實）

按 [[feedback_propose_v1_spec_landing_read_audit_first]] + [[feedback_exhaustive_sweep_first]]：

| 校準項 | 預期 | 實測 | 結論 |
|---|---|---|---|
| Spec wording | per-vault 明寫 | spec.md line 911 + 918 verbatim 確認 | impl 違反 spec |
| `ActiveRuns` 結構 | 含 vault 資訊 | `HashMap<String, Arc<AtomicBool>>`（line 19）只有 RunId | gap 確認 |
| `has_goal_run` impl | 取 vault scope | `keys().any(\|k\| !k.starts_with("chat-"))`（line 70-73）全 map 掃 | gap 確認 |
| Production consumer of `has_goal_run` / `has_chat_turn` / `has_quiz_run` | 全列舉 | `goals.rs:217`（1）+ `chats.rs:161, 294, 304, 380`（4）+ `quiz.rs:177, 314`（2）= 7 處 | 全 migrate scope 明確 |
| Production consumer of `active_runs.insert/remove/get` | 全列舉 | `goals.rs:231, 266, 294` + `chats.rs:174, 244, 317` + `quiz.rs:185, 322, 413` = 9 處（每 mode 三組 insert / remove / get；其中 chat reroute 邏輯 `chats.rs:294, 304, 380` 用 `has_chat_turn` 輪詢）| API 改動點全 mapping 完 |
| Frontend cleanup | reset 在 unmount 呼叫 | `Workspace.tsx:122-125` `goalsReset()` 在 useEffect cleanup return 內 | frontend 不是 bug |
| Chat spec per-vault wording | grep | `app-workspace § Chat Widget Session per vault`（line 1234）寫 session 是 per-vault，但對 active turn 跨 vault 行為 spec silent | chat 同步 vault-scope 是 conservative inference、非 spec 強制 |
| Quiz spec per-vault wording | grep | silent | quiz 同步 vault-scope 同 conservative inference |

### Apply-time 內部矛盾教訓（2026-05-28 user override 抓出）

Propose 階段除了 spec/impl 外部 gap、還要防 design Non-Goals 與 task 描述的**內部矛盾**。本 change Task 3.1 描述寫「cancel_goal 簽名加 vault_path」、但 design Non-Goals 寫「不改 IPC contract 對外 shape」——同份 artifact 自相矛盾、apply 啟動才抓到。

修法：propose 完成後 self-review 時對所有 task description 跑「contract grep」、確認與 Non-Goals / Decisions 不衝突；spectra analyzer 目前只查 Coverage/Consistency/Ambiguity/Gaps 跨 artifact 結構、不查跨 section 語意衝突。下次 propose 把這條加進 Inline Self-Review Check 6。

### 同名詞 disambiguation

承 [[project_quiz_fullscreen_wizard_view_term_disambiguation]] 教訓。「active goal」一詞跨三層不同意義、apply Task 1.1 必須先校齊：

| 詞 | 層 | 意義 |
|---|---|---|
| `activeRun` | frontend store field（`useGoalsStore`）| `{runId, goal, startedAt, events, cancelling}` 或 `null`；UI 用它擋 New Goal modal `Run` 按鈕（spec line 913） |
| `active_runs` | backend `AppRuntimeState` 欄位 | `ActiveRuns` struct、`Mutex<HashMap<...>>` 包裝、process-wide |
| 「Active Goal Run」 | spec 邏輯不變量 | 「at most one goal-mode `run_goal` invocation is active per vault per app session」 |

apply 階段不可在 task 描述把三層任一互換、否則 reviewer 對不上。

## Goals / Non-Goals

**Goals:**

- `ActiveRuns` impl 加 vault scope、`spawn_goal` cross-vault 不互擋、對齊 spec line 918
- API 命名同步表達 vault scope（`has_goal_run_for_vault` 取代 `has_goal_run`、避免 silent semantic change）
- chat / quiz 三 mode 用同 vault scope policy 套用（一致性 > 細節 spec 差異、spec silent 不阻擋 conservative inference）
- spec `One Active Goal Run At A Time` 補 cross-vault scenario、把 spec line 918 從散文 promise 升格為可測 scenario

**Non-Goals:**

- 不動 `ActiveRuns` 內部存儲（仍 `Mutex<HashMap<...>>`、不換 lock-free / sharded、premature optimization per 既有 doc comment）
- 不動 frontend store / Workspace lifecycle（unmount cleanup 已正確）
- 不修 hypothesis b'/c'/d'（orphan / 卡死 / frontend race）的 defensive 補丁、apply 若 reproduce 仍失敗再開獨立 change
- 不加 UX cross-route active-goal indicator（spec 允許 cross-vault、indicator 失去動機）
- 不擴張 spec 把 chat / quiz 明寫 per-vault（spec silent 留給未來 change、本 change 不擴張 spec scope）
- 不改 IPC contract 的對外 shape（`spawn_goal(vault_path, ...)` 簽名已含 vault_path、純內部 propagation；frontend 不需改）

## Decisions

### Decision 1 · Value-side vault storage（user override 2026-05-28 apply Task 3.x revised）

**Choice**（apply 修訂後）：`HashMap<RunId, ActiveRunEntry { vault_path, cancel }>`。RunId 仍是唯一 HashMap key、vault 進 value、cancel 路徑（`active_runs.get(run_id)` / `active_runs.remove(run_id)`）簽名完全不動。

**Original choice**（propose 階段、apply 時 revoked）：`HashMap<(VaultKey, String), Arc<AtomicBool>>` tuple key。原 task 3.1 描述把 cancel_goal IPC 簽名加 vault_path、與本 design Non-Goals「不改 IPC contract 對外 shape」直接矛盾——apply Task 3.x 啟動時抓到此 propose-time 內部矛盾、回 user 對齊、改採 value-side 方案。教訓進 Pre-apply 校準段。

**Rationale**（value-side）：
- IPC contract 完全不動：`cancel_goal(runtime, run_id)` / `cancel_chat_turn(runtime, run_id)` / `cancel_quiz_*` 簽名保留、frontend 不需改、與 Non-Goals 完全互恰
- Spawn 路徑只多一條 `vault_path` propagate：goals.rs / chats.rs / quiz.rs spawn site 在 `active_runs.insert(&vault_path, run_id, cancel)` 加一個 arg
- Cleanup 路徑（spawn 完成 thread terminus 的 `active_runs_thread.remove(&run_id)`）原樣 — RunId 即唯一 lookup key、無 vault propagate 義務
- Pre-spawn guard 改 vault-scoped：`has_*_for_vault(vault)` 走 `map.iter().any(|(run_id, entry)| entry.vault_path == vault && <prefix>)`、O(n) 但 n ≤ 數個 active run、cost negligible
- 結構直觀：`ActiveRunEntry { vault_path, cancel }` 把 vault 與 cancel flag 綁同一物件、語意對齊「這個 run 屬於哪個 vault + 怎麼取消」

**Alternatives**（apply 修訂後重新評估）：

- B. Tuple-key `(VaultKey, RunId)`（propose 原 plan）→ reject：cancel 路徑需 vault param、IPC contract 連動、frontend 跟改、blast radius 大、與 Non-Goals 衝突
- C. RunId 編碼 vault（`"<vault_hash>/<run_id>"`）→ reject：把 vault key 編進 RunId 等同污染 events log / wiki / RunLog 那些只關心 RunId 的下游；parse 脆弱、coupling 表示層
- D. `HashMap<VaultKey, HashMap<RunId, ...>>` 巢狀 → reject：跨 vault aggregation 寫起來醜、`is_empty()` 需 flatten
- E. 換 `dashmap` / 改 RwLock → reject：per Non-Goals premature optimization

### Decision 2 · 新增 `_for_vault` 後綴 API、舊 API 廢除

**Choice**：rename `has_goal_run() -> bool` 為 `has_goal_run_for_vault(vault: &str) -> bool`、`has_chat_turn` / `has_quiz_run` 同。舊版完全移除、不留 deprecated。

**Rationale**：
- 命名表達新 semantic（vault-scoped）、避免 silent semantic change 風險（同名不同行為 = 災難）
- 廢除而不 keep-old：grep 確認 production 7 consumer site 全已 mapping、移除舊 API 強迫 caller 對新 API 簽名（type system 是 safety net）
- 寫法與 acceptance criteria #6 / #7 一致（grep 0 hit on 舊名稱）

**Alternative**：保留 `has_goal_run() -> bool` 為「any vault」aggregate → reject：spec line 918 + 911 已明示 vault-scoped 是規範意圖、無 production caller 需 process-wide aggregate；保留 = 鼓勵未來 caller 誤用

### Decision 3 · Chat / Quiz 同 policy 套用、spec 不擴張 wording

**Choice**：chat / quiz 三 mode 的 `has_*` API 同步加 vault scope、IPC layer 同步改、但 spec wording 不加新 per-vault scenario。

**Rationale**：
- 一致性：三 mode 用同 ActiveRuns、結構統一比 mode-specific scope policy 易維護
- Spec 不擴張：goal spec 既有「per vault per app session」是 ground truth、chat / quiz spec silent 上 conservative inference（impl 行為跟 goal 一致）、不主動寫 spec 表達 chat per-vault 是因為「沒人 user-report bug、不需 propose 一個未實機驗的 spec」
- Risk acknowledged in Risks section

**Alternative**：
- 只動 goal、chat / quiz 留 process-wide → reject：mode-specific behavior 反而難 reason、且 chat ChatWidget 本身 per-Workspace mount、user 跨 vault 預期同 goal（一致 mental model）
- 同動且擴張 spec → reject：spec scope 擴張需獨立 propose / discuss、本 change 已塞滿

### Decision 4 · Spec delta 只加新 scenario、不改 requirement 本體

**Choice**：在 `One Active Goal Run At A Time` 之下加新 `#### Scenario: Cross-vault goal spawn allowed`、其他 4 scenario + requirement 正文不動。

**Rationale**：
- Spec line 918 散文已 promise cross-vault、補 scenario 是把 promise 升格為可測 contract、不算改 requirement 本體
- 既有 4 scenario 數值不動 = 既有 backend test 不破（accept criteria #8）
- 廢除「Idling in place」風險不存在（這 spec 沒同名詞 disambiguation 問題）

### Decision 5 · VaultKey 規範：caller 傳什麼 backend 用什麼

**Choice**：`vault: &str` API param 由 caller 端傳入、backend 不做 canonicalization / hash / normalize。

**Rationale**：
- Caller（`spawn_goal` IPC）已從 frontend 拿 `vault_path: String`、那 String 本身即 vault identity
- 若不同 caller 傳不同形式（絕對 path vs 相對 vs symlinked）會 mismatch、但 IPC layer 簽名都用 `vault_path: String`、frontend 來源都是 `vault.path`、實務上 caller 一致
- Backend canonicalization = scope creep（涉 FS access、可能 fail、引入 async / error handling）

**Trade-off**：如果未來 frontend 不同呼叫點傳不同形式 vault_path、會出現「同 vault 但 backend 認 2 個 key」的 bug。但這是 caller 義務、非 ActiveRuns 義務。Apply 階段加 unit test 文件化此假設。

### Decision 6 · `list_runs` 對 in-flight goal 回 `running` 而非 `interrupted`（apply Task 9.x 新增）

**背景**：apply 階段 grounded CDP smoke 真實重現 user-reported bug，抓到 **UI lie** 才是 user 真正痛點。`goals.rs:386-411` `list_runs_impl` 對「events JSONL 存在但 RunLog absent」的 orphan 一律合成 `outcome: "interrupted"`、不論該 entry 是否真的在 `active_runs` 跑著。User 切走再切回來 → frontend `activeRun` reset 為 null → list_runs 合成 interrupted → user 以為 goal 結束、嘗試 spawn 新 goal → backend 仍 reject「already active」→ frontend 沒 error 顯示 → 「Run 沒反應」。

**Choice**：`list_runs_impl` 簽名加 `active_runs: &ActiveRuns`、合成 orphan 迴圈內若 `active_runs.get(slug).is_some()` 改 `outcome: "running"`（保留 events JSONL 來源；finished_at 仍空）。Tauri command `list_runs` 對應加 `runtime: State<'_, AppRuntimeState>` Tauri injection。

**Rationale**：
- 真實 status 由 backend 唯一來源（`active_runs` map）決定、磁碟只當 fallback
- `outcome: "running"` 跟 frontend 既有 optimistic insert（goals.ts spawnGoal）相同字串、UI 已知道怎麼 render
- 不動 frontend 邏輯、純 backend 內部改 — 對齊 design 既有 Non-Goals「Out of scope: Frontend store / Workspace lifecycle」（仍 true、本 Decision 6 改 backend list_runs，不動 frontend store；frontend 自動受益）

**Alternative**：
- 加新 IPC `list_active_run_ids(vault) -> Vec<String>` 讓 frontend merge → reject：frontend 多一條 race / 多一次 IPC round-trip、coupling 不好
- 改 frontend `useGoalsStore.refreshRuns` 在 Workspace mount 時也 query backend active_runs 補 activeRun → reject：concerns scattered（既然 list_runs 已存在做 disk merge、加 active_runs merge 是同地方）

### Decision 7 · NewGoalModal 對 spawnGoal IPC reject 顯示 friendly error（apply Task 9.x 新增）

**背景**：Decision 6 修了 UI lie 後 user 不會再因錯覺去 spawn 新 goal、但**萬一仍命中 reject**（race condition / 跨 vault concurrent edge case / 未來 chat-quiz 同 vault rules 改變）、user 要看得到原因。原前端對 `await spawnGoalIpc` throw 沒視覺處理 → 「沒反應」。

**Choice**：NewGoalModal 的 submit handler 對 `spawnGoal` Promise catch、把 backend `AppError` 翻成 i18n message 顯示在 modal 內（紅字 inline、不關 modal、user 可 cancel / 改文本 retry）。預設 mapping：
- `"another goal run is already active"` → i18n key `goal.error.already_active`（zh-tw「上一個 goal 還在跑、請取消或等完成」/ en「Another goal is still running. Cancel it or wait for it to finish.」）
- 其他 backend error → i18n key `goal.error.spawn_failed_generic` + 詳細 message inline

**Rationale**：
- 保險網：Decision 6 修了主流 case、Decision 7 救剩下 edge case
- 不重新 design modal flow、只加 error state 與顯示
- i18n 對齊 codebus 既有 message pattern

**Alternative**：
- Toast / global notification → reject：modal 是 user 當前 focus context、inline error 最少 cognitive load
- 自動關 modal + 顯示 toast → reject：user 失去 modal 輸入的文字、重新打很煩

### Decision 8 · `refreshRuns` 於 Workspace remount 自 backend restore `activeRun`（apply Task 9.4 新增）

**背景**：apply 階段 user 自行真實操作 GUI 驗證 Decision 6 後抓到 follow-on bug。流程：spawn goal A → 切 Lobby（Workspace unmount → goalsReset → `activeRun=null`）→ 切回 vault A → list_runs 對 in-flight goal 正確回 `outcome: "running"`（Decision 6 ✓、UI list 顯示 running 綠點）→ **但 user 點該 row 進 RunDetail → 畫面全黑**。Root cause：RunDetail 從 `activeRun.events` 取 events 來 render；`activeRun` 已被 reset 清掉、沒有 events buffer → 黑畫面。既有 `_onStreamEvent` listener 會 append events、但 listener 只認 `activeRun.runId` match 的 stream payload；activeRun 是 null 時 events 全進 `tailByRunId`、不進 RunDetail 該顯示的緩衝區。

**Choice**：`useGoalsStore.refreshRuns(vaultPath)` 拿到 `list_runs` 結果後、若 `state.activeRun === null` 且 runs 內有 `outcome: "running"` 的 row → 用 `getRunDetail(vaultPath, runId)` 把 events JSONL 從磁碟讀回來、重建 `activeRun = { runId, goal, startedAt, events, cancelling: false }`。重建後既有 `_onStreamEvent` listener 自動 append 新進 events（runId match）、`_onTerminal` listener 也會在 goal 完成時 set null + refresh。

**Race guard**：detail fetch 是 async、期間 user 可能新 spawn 一個 goal（spawnGoal 已 set activeRun）。`set` callback 內檢查 `current.activeRun`、若已非 null 不覆寫、避免 race。

**Failure mode**：`getRunDetail` reject（events file 不見、IPC error）→ catch 後仍把 `runs` 寫回 store、`activeRun` 保持 null、RunDetail view 該自己處理 empty state。

**Rationale**：
- 修法在 frontend store、不動 backend（Decision 6 已 backend 補完、第 7 / 8 全在 frontend layer 補 UI 一致性）
- 用既有 `getRunDetail` IPC（不需新 IPC）
- 對齊 spec line 913「frontend `activeRun` is non-null when a spawn is in progress」— Workspace remount 後 in-progress goal 仍非 null
- 既有 `_onStreamEvent` / `_onTerminal` listener 機制原樣 work、無需動 listener registration

**Alternatives**：
- A. 改 RunDetail component 直接 query `get_run_detail` 而非從 store 讀 → reject：等同把 store 變成 cache + 加 RunDetail 內 fetch 邏輯、scattered concerns
- B. spawnGoal 改不在 unmount reset activeRun（讓它跨 Workspace mount 存活）→ reject：reset 既有設計（Workspace.tsx:122-125）、保留 vault-switch chat 等 state 一致性、不該為 goal 單獨破例
- C. 加新 backend IPC `list_active_runs(vault) -> Vec<RunSummary>` 給 frontend merge → reject：list_runs 已含此資訊（Decision 6 後）、不需第二 IPC

**Trade-off**：每次 Workspace remount 多一次 `getRunDetail` round-trip（僅當 backend 報 running 且 frontend 沒記憶時）— typical case 是 user 切走再回來、cost negligible。一般 spawn-and-stay-in-workspace case 不觸發。

### Behavior（user-observable）

- vault A spawn goal、不論 A goal 是否完成、user 切到 vault B 都能 spawn goal
- 同 vault 同 mode 互斥**完全不變**：vault A 跑 goal 時、再次 spawn goal in vault A 仍 reject（spec scenario `Second spawn_goal during active run rejected at backend` 保持）
- chat / quiz 同樣 per-vault scope：跨 vault 可同時跑、同 vault 同 mode 互斥
- 既有錯誤訊息文字 `"another goal run is already active"` 不變（spec line 914 抓 substring "already active"、UI 文案不動）

### Interface / data shape

- `ActiveRuns` struct：`pub struct ActiveRuns(pub Mutex<HashMap<String, ActiveRunEntry>>)`、key = `RunId`、value = `ActiveRunEntry { vault_path: String, cancel: Arc<AtomicBool> }`
- API（apply 修訂後）：
  - `insert(vault: &str, run_id: String, cancel: Arc<AtomicBool>)` — vault 進 value
  - `remove(run_id: &str)` — RunId 即唯一 key，無 vault param（cancel 路徑 / thread terminus 不變）
  - `get(run_id: &str) -> Option<Arc<AtomicBool>>` — 同 remove、回 entry.cancel
  - `has_goal_run_for_vault(vault: &str) -> bool`
  - `has_chat_turn_for_vault(vault: &str) -> bool`
  - `has_quiz_run_for_vault(vault: &str) -> bool`
  - `is_empty(&self) -> bool` 保留（cross-vault aggregate、test 仍用）
- 舊 API（無 `_for_vault` 後綴的 `has_goal_run` / `has_chat_turn` / `has_quiz_run`、單 arg 的 `insert(String, ...)` / `remove(&str)` / `get(&str)`）**完全移除**
- IPC caller 內部 propagate vault_path 到 active_runs 操作；IPC 對外簽名（frontend 看到的 Tauri command）**不變**

### Failure modes

- 若某個 IPC caller 漏改 vault_path propagate（傳空字串 `""` 或 stale value）→ unit test 補 vault mismatch 案例擋
- 若 vault_path 在不同 caller 不一致（絕對 vs 相對 path）→ caller bug、非 ActiveRuns 問題；apply Task 補 doc comment 警告
- 既有 `is_empty()` 仍可用（test 場景需 cross-vault aggregate）、production code 不依賴它

### Acceptance criteria

完整列表見 proposal.md `## Success Criteria`、共 8 條。最關鍵驗證點：

1. Static：grep `has_goal_run\b` / `has_chat_turn\b` / `has_quiz_run\b`（不含 `_for_vault`）於 src-tauri/src/ = 0 hit（success criteria #6 / #7）
2. Static：spec.md 新 scenario `Cross-vault goal spawn allowed` 存在、`spectra validate` 綠（#5）
3. Dynamic：`cargo test` 全綠、含新加 per-vault unit test（#4）
4. Dynamic：`pnpm tsc` + `pnpm test` 全綠（frontend 不該動、確認回歸）
5. Dynamic：CDP smoke 三次連跑 vault A → lobby → vault B spawn goal 全綠（#1）+ 並行 cross-vault case（#2）
6. Spec scenario 既有 4 條對應 unit test 仍綠（#8）

### Scope boundaries

**In scope**:

- `codebus-app/src-tauri/src/state/active_runs.rs`：struct + API 全 migrate
- `codebus-app/src-tauri/src/ipc/goals.rs`：line 217 / 231 / 266 / 294 callsite migrate + 既有 test 補 vault param + **`list_runs_impl` 加 active_runs param 並對 in-flight orphan 標 `running`**（Decision 6、apply 新增）
- `codebus-app/src-tauri/src/ipc/chats.rs`：line 161 / 174 / 244 / 294 / 304 / 317 / 380 callsite migrate
- `codebus-app/src-tauri/src/ipc/quiz.rs`：line 177 / 185 / 314 / 322 / 413 callsite migrate
- `openspec/specs/app-workspace/spec.md`：`One Active Goal Run At A Time` requirement 加新 scenario + `Interrupted Run Detection` 補 active-runs-aware 行為
- **`codebus-app/src/components/workspace/NewGoalModal.tsx`**：spawnGoal IPC reject 顯示 inline friendly error（Decision 7、apply 新增）
- **`codebus-app/src/i18n/messages.ts`**：加 `goal.error.already_active` / `goal.error.spawn_failed_generic` zh + en
- Test：active_runs.rs `#[cfg(test)]` mod 補 per-vault cases + goals.rs cross-vault integration test + goals.rs `list_runs` 對 active-runs aware unit test + NewGoalModal error display 補 test

**Out of scope**:

- Frontend `useGoalsStore` lifecycle（reset 在 unmount 仍正確；本 change 不動）
- Workspace.tsx unmount cleanup（仍正確）
- IPC Tauri command signature（對外 surface）— vault_path 已是 param、內部 propagation 是 implementation detail；**例外**：`list_runs` 內部加 `runtime: State<'_, AppRuntimeState>` Tauri inject（per Decision 6、frontend 簽名不變、IPC param 不變）
- Vault path canonicalization（caller 義務）
- Hypothesis b' / c' / d' defensive 補丁
- `app-workspace` spec 其他 requirement
- chat / quiz spec wording 擴張（spec silent 留現狀、design intent 寫進本 design.md）

## Risks / Trade-offs

- **Risk**: chat / quiz 改 per-vault scope 是 conservative inference、spec silent；若未來 chat / quiz spec 寫成「跨 vault 也要互斥」（e.g. quiz 因 token cost 想全 process 限制）會 conflict → **Mitigation**: design.md Decision 3 紀錄此推論基礎、未來 spec 若擴張規範可獨立改 ActiveRuns；本 change 假設「一致性 > 邊際細節」
- **Risk**: VaultKey 用 raw `&str` 無 canonicalization、caller 不同形式 vault_path 會 mismatch → **Mitigation**: 既有 IPC frontend → backend 走同 source（`vault.path`）、production caller 一致；apply Task 補 doc comment + 單測一個「different path strings same logical vault」案例 + 文件化「caller 義務 canonicalize」
- **Risk**: `is_empty()` 仍是 cross-vault aggregate、test 可能誤用為「特定 vault 沒 active」→ **Mitigation**: doc comment 標 `Cross-vault aggregate; production code SHOULD prefer has_*_for_vault`、unit test 加註解
- **Risk**: 既有 spec scenario `Second spawn_goal during active run rejected at backend` 用 "vault `V`" 抽象、本 change 把它具象化為「同一 vault」、其他既有 test 可能默契假設 process-wide → **Mitigation**: acceptance criteria #8 強制既有 test 仍綠、breaks 即發現
- **Trade-off**: 不寫 hypothesis b'/c'/d' defensive 補丁 = 賭 a' 修完就好；若 user reproduce 仍見 bug、需獨立 change 追、apply 階段做 fresh CDP smoke 證 a' 修完 user-report 消失（per [[feedback_grounded_debugging]]）
- **Trade-off**: 不留舊 API（無 `_for_vault` 後綴）= type system breaking change、強迫 callsite 改；好處是 grep 0 hit 可驗 0 漏網、壞處是 git diff 較大；接受

## Migration Plan

純內部 impl 改動、無 data migration、無 IPC 對外 contract change。

Apply 步驟（依 tasks.md）：

1. active_runs.rs：struct + API 全改、unit test 補
2. goals.rs：callsite migrate + 既有 test 補 vault param
3. chats.rs：callsite migrate
4. quiz.rs：callsite migrate
5. cargo build + cargo test green
6. Spec scenario 補
7. spectra validate
8. Frontend tsc + test 確認無回歸
9. CDP smoke 驗 cross-vault behavior
10. Report

Rollback：git revert（無 persistent state migration、無外部 contract change）

## Open Questions

無。Decision 1-5 已收斂、user-approved hypothesis a' 唯一動工方向。
