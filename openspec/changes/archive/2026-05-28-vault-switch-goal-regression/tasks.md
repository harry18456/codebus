<!--
Behavior + verification per task. File paths are locator context.
parallel_tasks: true → [P] marker on tasks targeting different files with no shared dep.
tdd: true → 先寫 failing test 再 impl，per active_runs.rs / goals.rs 在同 mod #[cfg(test)] 内逐 case 處理；spec scenario 也作為 test source。
-->

## 1. Pre-apply ground-truth recheck + disambiguation

- [x] 1.1 重跑 Pre-apply 校準（grep + Read 證實）+ 同名詞 disambiguation 校齊：用 Grep 重驗 design.md Context 表格 8 項仍成立——spec.md line 911 + 918 verbatim 未變、`ActiveRuns` 仍是 `HashMap<String, Arc<AtomicBool>>` 無 vault key、`has_goal_run` / `has_chat_turn` / `has_quiz_run` 三 API 共 7 production consumer site（goals.rs:217、chats.rs:161/294/304/380、quiz.rs:177/314）、`active_runs.insert/remove/get` 9 個 production site、Workspace.tsx:122-125 `goalsReset()` 仍在 unmount cleanup。同時對 chat 把「active goal」三層 disambiguation 表念出（frontend `activeRun` / backend `active_runs` / spec 邏輯不變量）、apply 後續 task 不互換。驗證：Grep 結果各自 echo 出來、任一項與 design.md 表不符立刻 stop 對齊 design。

## 2. ActiveRuns struct + API per-vault scope（TDD 啟動）

- [x] 2.1 在 `codebus-app/src-tauri/src/state/active_runs.rs` 的 `#[cfg(test)]` mod 加 failing test：`active_runs_cross_vault_goal_allowed`、`active_runs_same_vault_same_mode_blocks`、`active_runs_per_vault_chat_and_quiz_isolation`、`active_runs_get_requires_matching_vault`、`active_runs_is_empty_aggregates_across_vaults`、`active_runs_remove_only_targets_matching_vault_run`。每 test 對應 Decision 1 / 2 / 5 與 spec ADDED scenarios。驗證：`cargo test -p codebus-app-tauri --lib active_runs` 跑出新 test 全 fail（pre-impl baseline、確認 test 真在驗新行為）。

- [x] 2.2 落地 Decision 1 · ActiveRuns key 改 (VaultKey, RunId) tuple + Decision 2 · 新增 `_for_vault` 後綴 API、舊 API 廢除（含 Decision 5 · VaultKey 規範：caller 傳什麼 backend 用什麼）+ 落地 Interface / data shape（新 API 簽名表）：把 `ActiveRuns` 內 `HashMap<String, _>` 改 `HashMap<(String, String), _>`、`insert` / `remove` / `get` 全加 `vault: &str` first param、刪除 `has_goal_run` / `has_chat_turn` / `has_quiz_run` 三個無後綴 API、新增 `has_goal_run_for_vault(&str) -> bool` / `has_chat_turn_for_vault(&str) -> bool` / `has_quiz_run_for_vault(&str) -> bool`（raw `&str`、不 canonicalize、caller 義務）、`is_empty()` 保留並加 doc comment「Cross-vault aggregate; production code SHOULD prefer has_*_for_vault」。驗證：`cargo build -p codebus-app-tauri` 綠（callsite 還沒改、預期破、看編譯訊息 7 + 9 site 全列出來確認 sweep 對得上 design.md Pre-apply 表）→ task 2.3 之後才會 green。

- [x] 2.3 active_runs.rs 既有 unit test 補 vault param（既有 `active_runs_insert_then_remove` / `active_runs_remove_unknown_id_is_noop` / `active_runs_get_unknown_returns_none` / `has_chat_turn_detects_chat_prefix` / `has_goal_run_ignores_chat_prefix` / `has_quiz_run_detects_quiz_prefix` / `has_quiz_run_false_for_chat_and_goal_ids` / `chat_and_goal_can_coexist` 共 8 個）改用新 API 簽名、行為斷言不動。驗證：`cargo test -p codebus-app-tauri --lib active_runs` 對應 8 + 6 = 14 個 test 全綠。

## 3. IPC layer migrate consumer

- [x] 3.1 [P] `codebus-app/src-tauri/src/ipc/goals.rs` 全 callsite migrate（value-side vault storage、user override 2026-05-28、IPC contract 不動）：spawn_goal pre-spawn guard 改 `active_runs.has_goal_run_for_vault(&vault_path)`、`active_runs.insert(&vault_path, run_id, cancel)` 加 vault arg、`active_runs_thread.remove(&run_id)` 與 `cancel_goal_impl` 的 `active_runs.get(run_id)` **簽名不變**（value-side 設計、cancel 路徑只看 run_id）。實作完同檔內既有 `#[cfg(test)]` mod 的 test 函式（包含 line 695 / 701 / 707 / 720 / 753 / 771 / 775 / 782 / 800 等）：insert 加 vault arg、get/remove 不變、`has_chat_turn()` 替 `has_chat_turn_for_vault(&vault_path)`。驗證：`cargo test -p codebus-app-tauri --lib goals` 全綠、Grep `has_goal_run\b\|has_chat_turn\b\|has_quiz_run\b`（無 _for_vault 後綴）於 src-tauri/src/ipc/goals.rs = 0 hit。

- [x] 3.2 [P] `codebus-app/src-tauri/src/ipc/chats.rs` 全 callsite migrate（Decision 3 · Chat / Quiz 同 policy 套用、spec 不擴張 wording、value-side IPC 不動）：spawn_chat_turn pre-spawn guard 改 `has_chat_turn_for_vault(&vault_path)`（line 161）、`active_runs.insert(&vault_path, ...)` 加 vault arg（line 174 + 317）、`cancel_chat_turn` 的 `active_runs.get(run_id)` **簽名不變**（line 244、value-side 設計）、chat reroute 邏輯 line 294/304/380 的 `has_chat_turn()` 全替 `has_chat_turn_for_vault(&vault_path)`、同檔內 test 函式（line 430-475 附近）對應補正、`assert!(!active_runs.has_goal_run(),...)` 行（line 468）替成 `has_goal_run_for_vault(&vault_path)`。驗證：`cargo test -p codebus-app-tauri --lib chats` 全綠、Grep `has_chat_turn\b`（無 _for_vault 後綴）於 src-tauri/src/ipc/chats.rs = 0 hit。

- [x] 3.3 [P] `codebus-app/src-tauri/src/ipc/quiz.rs` 全 callsite migrate（Decision 3 同 policy、value-side IPC 不動）：spawn_quiz_plan / spawn_quiz_generate pre-spawn guard 改 `has_quiz_run_for_vault(&vault_path)`（line 177 + 314）、`active_runs.insert(&vault_path, ...)` 加 vault arg（line 185 + 322）、cancel 路徑 `active_runs.get(run_id)` **簽名不變**（line 413）、同檔內既有 quiz test 對應補正（insert 加 vault arg）。驗證：`cargo test -p codebus-app-tauri --lib quiz` 全綠、Grep `has_quiz_run\b`（無 _for_vault 後綴）於 src-tauri/src/ipc/quiz.rs = 0 hit。

- [x] 3.4 跨檔 sweep（exhaustive、per [[feedback_exhaustive_sweep_first]]）：Grep `has_goal_run\b\|has_chat_turn\b\|has_quiz_run\b`（無 _for_vault 後綴、line-anchored 字邊界）於整個 `codebus-app/src-tauri/src/` 與 `codebus-app/src-tauri/tests/` = 0 hit（active_runs.rs / goals.rs / chats.rs / quiz.rs 都已 task 2-3 處理完、本步是 paranoia 保險）。同時 Grep `active_runs\.insert\(` 看 spawn 路徑簽名 — 結果全部都應 vault + run_id + cancel 三 args；`active_runs\.(get|remove)\(` 看 cancel/cleanup 路徑 — 全部都應僅 run_id 單 arg（value-side 設計、IPC contract 保護）。驗證：兩條 Grep 結果各自 echo 出來。

## 4. Cross-vault integration test（goals.rs）

- [x] 4.1 在 `codebus-app/src-tauri/src/ipc/goals.rs` 既有 `#[cfg(test)]` mod 加 integration test `spawn_goal_cross_vault_allowed`：mock runner、模擬 vault A 已有 goal-mode 進 active_runs、然後 spawn_goal_impl 對 vault B 呼叫、assert 回 Ok(run_id) 且 active_runs 同時含 vault A + vault B 的 goal entry。對應 spec ADDED scenario「Cross-vault goal spawn allowed while another vault has an active goal」。驗證：新 test `cargo test -p codebus-app-tauri --lib goals::tests::spawn_goal_cross_vault_allowed` 綠、舊 test `spawn_goal_rejects_when_active` / `cancel_goal_flips_flag_when_present`（line 695 / 800 附近、name 自尋）仍綠。

## 5. Spec landing

- [x] 5.1 在 `openspec/specs/app-workspace/spec.md` `Requirement: One Active Goal Run At A Time` 區段（line 909-918 區域）後、`<!-- @trace ... -->` 區段（line 936）前、加新 `### Requirement: Cross-Vault Goal Spawn Permitted` 含 4 個 scenario（per change spec delta `specs/app-workspace/spec.md` MODIFIED 區、Decision 4 · Spec delta 只加新 scenario、不改 requirement 本體）。同步加 `<!-- @trace source: vault-switch-goal-regression / updated: 2026-05-28 -->` block 引用 ipc/goals.rs + ipc/chats.rs + ipc/quiz.rs + state/active_runs.rs 為 code + 對應 test 為 tests。驗證：Grep `Requirement: Cross-Vault Goal Spawn Permitted` 於 `openspec/specs/app-workspace/spec.md` = 1 hit、`Requirement: One Active Goal Run At A Time` 仍 1 hit（保留原 requirement）、`spectra validate vault-switch-goal-regression` 綠。

## 6. Build + test verification（Acceptance criteria）

- [x] 6.1 跑 `cargo test -p codebus-app-tauri` 整 crate test 全綠（Acceptance criteria #4）。驗證：command 退出 code = 0、output 列出新加 6 + 1 = 7 個 per-vault test 全 passed。

- [x] 6.2 跑 `cargo build -p codebus-app-tauri --release` 驗 release build 也綠（catch debug-only 編譯成功的 case）。驗證：command 退出 code = 0。

- [x] 6.3 跑 `pnpm tsc` + `pnpm test` 於 `codebus-app/` 確認 frontend 無回歸（本 change 不該動 frontend、若 frontend 任一 test 失敗即代表 IPC 對外簽名意外破）。驗證：`pnpm tsc` EXIT=0、`pnpm test` 全 file 全綠。

- [x] 6.4 跨檔最後 sweep：Grep `has_goal_run\b\|has_chat_turn\b\|has_quiz_run\b`（無 _for_vault 後綴）於 codebus-app/src-tauri/ 整目錄（含 src/ + tests/）= 0 hit、Grep `_for_vault\b` 同目錄 ≥ 1 hit each for 三個 mode。驗證：Grep 結果命中 0 + 3 各自 echo。

## 7. CDP smoke verification（user-observable behavior）

- [x] 7.1 跑 dev server `pnpm tauri dev`（user 啟、9222 port 開）、用 `codebus-app/scripts/cdp.mjs` connectOverCDP 連線、執行 sequence A（spec ADDED scenario「Cross-vault goal spawn allowed while another vault has an active goal」對應 user flow、對應 design.md Behavior（user-observable）第 1 條 + Failure modes 中「vault_path propagate 漏」風險）：
  - 透過 `__codebus_test_add_vault__` add vault A（真 temp dir）→ open vault A → click「+ New goal」→ submit goal → wait completion → 確認 backend `active_runs` empty（透過新加 dev-only IPC 或從 cargo log 取代）
  - back to Lobby → add vault B → open vault B → click「+ New goal」→ submit goal → spawn 成功
  - 重複 sequence 3 次連跑、每次成功

  驗證：3 次 sequence 都 spawn 成功、無 `another goal run is already active` error。截圖至 `codebus-app/scripts/.vault-switch-goal-regression-smoke/seq-1-3/`。

- [x] 7.2 CDP smoke 跑 sequence B（並行 cross-vault、Acceptance criteria #2）：
  - add vault A + add vault B
  - open vault A → spawn long-running goal（goal text 故意冗長）→ 不等完成
  - 立刻 back to Lobby → open vault B → spawn goal in B
  - vault B spawn 應成功（不 reject）、vault A goal 仍在跑、active_runs 同時有兩 entry

  驗證：vault B spawn 回 Ok、vault A 的 goal 流仍 emit goal-stream event（透過 frontend store 觀察）、截圖至 `.vault-switch-goal-regression-smoke/seq-B/`。

- [x] 7.3 CDP smoke 跑 negative case（既有 4 scenario 仍 true）：同 vault 同 mode 仍互斥：
  - vault A → spawn goal → 立刻再 spawn goal in vault A → reject with "already active"
  - vault A → spawn goal → spawn chat in vault A → 應允許（chat 與 goal 同 vault 共存）

  驗證：第一 case reject 且 error message 含 "already active"；第二 case chat spawn Ok。截圖至 `.vault-switch-goal-regression-smoke/seq-negative/`。

## 8. Final validation + report

- [x] 8.1 跑 `spectra validate vault-switch-goal-regression` 確認 spec delta 結構 + tasks 對應通過。驗證：command 退出 code = 0、output 無 Critical / Warning。

- [x] 8.2 停在 archive 前對 user report：列出本 change 所有 modified file（design.md 中「Scope boundaries In scope」對表）、grep 證據（Acceptance criteria #4-#8 對表）、smoke 截圖路徑、新加 spec scenario 名、新 API 名稱與舊 API grep 0 hit 證明。報告完等 user 決定 archive + commit（per [[feedback_archive_commit_immediately_after_apply]] 不自作主張）。驗證：report 訊息在 chat 內、未執行 `spectra archive` 或 `git commit`。

## 9. Ingest expansion · UI lie + spawn-error display（apply 2026-05-28 grounded reproduce 後）

User 真實 bug 重現後（spawn goal → 切 Lobby → 切回來 → UI 顯示 interrupted、再 spawn → Run 沒反應）抓到本 change 原 scope 沒涵蓋的兩條 root cause；ingest 進 Decision 6 / 7、補實作 + 驗證。

- [x] 9.1 落地 Decision 6 · `list_runs` 對 in-flight goal 回 `running` 而非 `interrupted`：`codebus-app/src-tauri/src/ipc/goals.rs` 中 `list_runs_impl` 簽名加 `active_runs: &ActiveRuns` param、合成 orphan 迴圈內若 `active_runs.get(slug).is_some()` 把 outcome 改 `"running"`（保留 events JSONL 來源 + goal_text）；Tauri command `list_runs` 對應加 `runtime: State<'_, AppRuntimeState>` Tauri inject、IPC 對外 payload shape 不變。同檔內既有 list_runs test（`list_runs_synthesizes_interrupted_virtual_entry` / `list_runs_synthesizes_interrupted_only_for_goal_events` 等）對應補 active_runs param。新增 unit test `list_runs_marks_in_flight_goal_as_running_when_active_runs_has_it`：mock active_runs 插一個 slug、events file 寫對應 banner、list_runs 回的 entry outcome 該為 `running` 而非 `interrupted`。驗證：`cargo test -p codebus-app-tauri --lib goals::tests::list_runs` 全綠、新 test passed。

- [x] 9.2 落地 Decision 7 · NewGoalModal 對 spawnGoal IPC reject 顯示 friendly error：`codebus-app/src/i18n/messages.ts` 加 zh-tw + en pair `goal.error.already_active`（zh-tw「上一個 goal 還在跑、請取消或等完成」/ en「Another goal is still running. Cancel it or wait for it to finish.」）+ `goal.error.spawn_failed_generic`（zh-tw「無法起新 goal: {message}」/ en「Failed to start goal: {message}」）；`codebus-app/src/components/workspace/NewGoalModal.tsx` 加 inline error state、submit handler `await spawnGoal(...)` 包 try-catch、catch 路徑判斷 error message 含 "already active" → 顯示 already_active i18n、其他 → 顯示 spawn_failed_generic + raw message、modal 不關（user 可改文本 retry 或 cancel）。驗證：對應 NewGoalModal test 補 spawn-reject case（mock spawnGoal throw、assert 紅字 inline 顯示、modal 仍 open）、`pnpm test src/components/workspace/NewGoalModal.test.tsx` 綠。

- [x] 9.3 spec 補 `app-workspace § Interrupted Run Detection` 為 active-runs-aware：openspec/specs/app-workspace/spec.md 找 `Requirement: Interrupted Run Detection`（若存在）modify 加新 scenario「In-flight goal in active_runs SHALL surface as outcome=running not interrupted」；若不存在這條 requirement，建新 `Requirement: In-Flight Goal Surface As Running In List Runs` 含對應 scenario。驗證：spectra validate 綠、Grep 新 scenario name 於 spec.md = 1 hit。

## 10. CDP smoke re-verify · 真實 user flow

- [x] 10.1 CDP smoke 完整重跑 user-reported flow（baseline post-fix）：
  - add vault A（temp dir）
  - 進入 vault A workspace
  - 透過 IPC 直接 spawn_goal in vault A、拿 runId
  - 截圖 vault A workspace 含 goal row、確認 outcome 顯示 `running`（非 interrupted）
  - 按 Back to Lobby
  - 等 ~1s
  - 重 open vault A
  - 截圖 vault A workspace、確認 goal row outcome 仍 `running`（**Decision 6 驗證點**、非 interrupted lie）
  - 嘗試開 New Goal modal、submit、看 Run 行為（若 backend 仍 active 該被 reject + inline error 顯示、**Decision 7 驗證點**）
  - cleanup：cancel runId、remove vault A

  驗證：兩張截圖內 goal row outcome 都為 running、NewGoalModal 失敗時顯示 friendly error 文字（不是「沒反應」）。截圖存 `codebus-app/scripts/.vault-switch-goal-regression-smoke/user-flow-fix/`。

- [x] 10.2 跑 `cargo test` 全 lib + `pnpm tsc` + `pnpm test` 確認 Decision 6/7 不破其他、`spectra validate vault-switch-goal-regression` 綠。驗證：三條 command exit 0。

## 11. Final report (re-do)

- [x] 11.1 對 user 重新 report：(1) 真正解決什麼（UI lie + spawn-reject silent）+ (2) backend per-vault scope 是 bonus alignment、未替 user 解 report 但獨立成立 + (3) 列所有 modified file + grep 證據 + smoke 截圖、等 user 決定 archive + commit。驗證：訊息在 chat、未自作主張 archive / commit。

## 12. Decision 8 · Restore activeRun on Workspace remount（apply 11.1 後 user 真實操作抓到 follow-on bug ingest）

User 在 11.1 report 後親自試走 user-reported flow、抓到 Decision 6 修了 UI list 顯示 running 綠點 ✓ 但**點進 row 進 RunDetail 全黑**、因為 frontend `activeRun` 是 null、RunDetail 從 `activeRun.events` 讀不到 buffer。

- [x] 12.1 落地 Decision 8 · `refreshRuns` 補 restore activeRun：`codebus-app/src/store/goals.ts` 的 `refreshRuns(vaultPath)` 拿到 `list_runs` 結果後、若 `state.activeRun === null` 且 runs 內有 `outcome: "running"` 的 row → 用 `getRunDetail(vaultPath, runId)` 把 events 從磁碟讀回來、重建 `activeRun = { runId, goal, startedAt, events: detail.events.map(env => env.event), cancelling: false }`。`set` callback 內 race guard 檢查 `current.activeRun`、若已非 null 不覆寫（避免覆蓋同時 spawn 的新 activeRun）。`getRunDetail` reject 走 catch、保持 activeRun null、只 publish runs。同步在 `codebus-app/src/store/goals.test.ts` 加三個 test：(1) restores activeRun from get_run_detail when backend reports running and frontend forgot、(2) does NOT restore when backend has no running row、(3) falls back gracefully when get_run_detail rejects。驗證：`pnpm test src/store/goals.test.ts` 15/15 全綠（12 既有 + 3 新）、`pnpm tsc` EXIT=0、`pnpm test` 全 file 1165/1165 綠。
