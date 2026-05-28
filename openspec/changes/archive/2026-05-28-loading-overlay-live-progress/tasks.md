<!--
每個 task 必須陳述「交付的 behavior / contract」+「驗證手段」。
File path 只是定位用，「edit file X」不算 task。
TDD 模式：tests 先寫、再寫 implementation。
[P] = 與同群組其他 [P] task 無檔案衝突、可並行。
-->

## 1. Pre-apply 校準與 ground truth lock

- [x] 1.1 對 design.md「Pre-apply 校準（grep 結果）」段做最後一次實機 grep：確認 `InitEvent` 22 個 variant 名與 design.md「Backend 結構校準」+「Frontend 結構校準」清單 1:1 對齊、`StepDots` 仍是 `QuizTab.tsx` local function、`--color-warn` 仍是 `tokens.css:26` 的 amber-warm token、`vault_list.rs` 兩個 `|_| {}` noop seam 仍存在。驗證：執行 `rg "InitEvent::" codebus-core/src/vault/init.rs` 數 variant、`rg "StepDots" codebus-app/src` 確認只有 `QuizTab.tsx`、`rg "color-warn" codebus-app/src/styles/tokens.css` 命中，diff 出與 design.md 不符之處逐條記回 design.md「AUDIT 與實機差異（必須在 apply 階段對齊）」表。
- [x] 1.2 對 design.md「同名詞 disambiguation」做最後校準：確認本 change 引用 `loading` / `phase` / `step dots` / `phase dots` / `InitEvent` 五詞時都符合該表定義；若 spec / tasks 內任何處有歧義，回頭改名統一。驗證：人工 review spec.md + tasks.md 全文搜尋上述五詞使用一致。
- [x] 1.3 對 design.md「serde tag collision 校準」做最後驗證：新 struct `VaultInitProgress` 的欄位名（`phase` / `init_event_kind` / `elapsed_ms`）不與既有 `#[serde(tag = "kind")]` 衝突、序列化後 JSON 鍵名仍是 snake_case。驗證：apply 階段寫單元 test 對 `VaultInitProgress { phase: 3, init_event_kind: "NestedRepoDone".into(), elapsed_ms: 1200 }` 做 serde round-trip，assert JSON 為 `{"phase":3,"init_event_kind":"NestedRepoDone","elapsed_ms":1200}`。

## 2. Backend：phase mapping 模組與測試（先 test、後實作）

- [x] 2.1 在 `codebus-app/src-tauri/src/ipc/vault_progress.rs`（新檔）寫 `init_event_to_phase` 6-phase mapping 的單元 test：每個 InitEvent variant 至少一個 case，斷言回傳值與 design.md「Phase Mapping Table（authoritative，實機 variant 名為準）」段 + spec「Vault Init Progress Event」requirement 的對應表完全相同。實作此 mapping 函式（per design「Phase mapping 邏輯放 Tauri layer 而非 codebus-core」決策、`match` 全窮舉、不留 catch-all arm）。驗證：`cargo test -p codebus-app vault_progress` 綠、把任一 InitEvent variant 從 match 移除時 `cargo build` 報 `non-exhaustive patterns` 錯誤（手動驗一次）。
- [x] 2.2 在 `vault_progress.rs` 定義 `VaultInitProgress` 序列化 struct（per design「Tauri Event Contract」段欄位定義 `phase: u8` / `init_event_kind: String` / `elapsed_ms: u64`、加 `#[serde(rename_all = "snake_case")]`、加 `#[derive(Serialize, Clone)]`）。寫 serde round-trip test 驗 JSON 鍵名為 snake_case。驗證：`cargo test -p codebus-app vault_progress::tests::payload_serde` 綠。

## 3. Backend：`add_vault_at` async cascade（per design「add_vault_at sync to async + accept AppHandle」）

- [x] 3.1 把 `codebus-app/src-tauri/src/ipc/vault_list.rs` 的 `add_vault_at` 從 sync 改 `async fn`、簽名加 `app: &tauri::AppHandle` 第一個參數；同檔內 `add_vault` IPC command 更新呼叫（`.await` + 轉發 `AppHandle`）。驗證：`cargo build -p codebus-app` 綠（warning 0）、`cargo clippy -p codebus-app` 不出新 warning。
- [x] 3.2 替 `vault_list.rs` 內現有 4 個 `add_vault_at` unit test caller（`fresh init`、`require mode`、`just_bind`、`reject missing`、`first/second add`、`bind ok`）改用 `tauri::test::mock_app()` 取 `AppHandle` + `tauri::async_runtime::block_on` 包覆，或 extract 不需 init 的 pure path 給純 vault-list 測試使用。確保「Just-bind mode emits no progress events」與「Detect-mode add emits one event per InitEvent」spec scenario 都有對應 Rust 端測試。驗證：`cargo test -p codebus-app vault_list` 全綠、新 mock helper `run_add_vault_at_in_test` 在 4 個 caller 一致使用。
- [x] 3.3 在 `vault_list.rs` 兩處 `run_init(..., |_| {})` noop seam（`AddVaultMode::Detect` 與 `AddVaultMode::ReInit`）改成 emit `vault-init-progress` Tauri event（per design「Tauri event 而非擴展 IPC command 回傳」決策）：closure 把 `InitEvent` 過 `init_event_to_phase` 轉 phase、組 `VaultInitProgress { phase, init_event_kind: event.label().into(), elapsed_ms: started.elapsed().as_millis() as u64 }`、用 `app.emit("vault-init-progress", &payload)` 發。`elapsed_ms` 從 `add_vault_at` 入口 `Instant::now()` 起算。驗證：寫 happy-path async test，攔 `app.listen_global("vault-init-progress")` 收集 event、`cargo test -p codebus-app vault_list::tests::detect_emits_progress_events` 斷言收到 21 個以上 event 且 phase 序列遞增不倒退 + elapsed_ms 單調非遞減。

## 4. Frontend：PhaseDots 元件 extract（per design「Phase dots 元件 extract 而非複製」）

- [x] 4.1 新建 `codebus-app/src/components/PhaseDots.tsx`：props `{ total: number; current: number; state?: "running" | "done" | "error"; testId?: string; currentAttrName?: string }`，渲染 `total` 個 `<span class="...">` dot；`current - 1` 個 done dot、第 `current` 個 active dot（state `"error"` 時用 `bg-warn` token、否則 `bg-accent ring-2 ring-accent-tint`）、其餘空 dot；root span 帶 `data-testid={testId}` + `data-{currentAttrName}={current}`。寫 `PhaseDots.test.tsx`：assert 4-dot 第 2 dot active、6-dot 第 5 dot active、`state="error"` 時 active dot 帶 `--color-warn`。驗證：`pnpm test PhaseDots` 綠。
- [x] 4.2 把 `codebus-app/src/components/workspace/QuizTab.tsx` 內 `function StepDots` 換成 `<PhaseDots total={4} current={current} testId="quiz-wizard-step-dots" currentAttrName="current-step" />`；既有 `quiz-wizard-step-dots` testid 與 `data-current-step` attribute 不變（spec scenario「Quiz wizard step dots continue to work」）。驗證：`pnpm test QuizTab` 既有 4 個 step dots assertion 不退化、`pnpm test forbidden-behaviors` 不破。

## 5. Frontend：LoadingOverlay 6 phase state machine（per spec「LoadingOverlay Live Progress」+ design「Observable Behavior」/「Minimum 300ms per phase」/「Fallback path」）

- [x] 5.1 新建 `codebus-app/src/components/LoadingOverlay.test.tsx`：先寫所有 spec scenario 對應的 failing test —— 「Initial mount shows fallback before any event」（phase=0、render `loading.subtitle`、無 dots）/「Phase advances on event」（phase 1→2 動態切換、bus DOM 節點 identity 同一個）/「Backend skips phase 5 but UI still pauses」（minimum 300ms timer 用 fake timers 驗）/「Successful finish fades out」（200ms opacity transition 後 unmount）/「Backend never emits events but IPC succeeds」（fallback path 直接 fade-out、無 error UI）。驗證：所有 test 在實作前 fail（red phase）。
- [x] 5.2 改寫 `codebus-app/src/components/LoadingOverlay.tsx`：用 `useEffect` + `@tauri-apps/api/event` `listen("vault-init-progress", ...)` 訂閱 event；state `{ phase: number; failed: boolean; lastEnteredAt: number; queuedPhase: number | null; localizedError: LocalizedError | null }`；切 phase 邏輯用 minimum 300ms timer（per design「Minimum 300ms per phase（所有 phase 同 strategy）」決策）；finished 時 fade-out 200ms 後 unmount（透過 `useVaultsStore` 既有 `initInProgress` 機制 + 內部 `visible` state、200ms transition 後 set initInProgress=false 觸發 unmount）；phase 0 時走 design「Fallback path（backend event 沒到）」段定義的 fallback render（v1 `loading.title` + `loading.subtitle`、不渲染 dots）。驗證：5.1 寫的所有 test 由 fail 變綠（green phase）、`pnpm tsc` 綠、bus emoji DOM 節點在 phase 1→6 中 identity 不變（jest assertion）。

## 6. Frontend：失敗模式與慢階段 hint（per design「Failure mode 用 amber-warm 而非 hard-fail red」）

- [x] 6.1 在 `LoadingOverlay.test.tsx` 加 failure mode test：「Backend error enters failure mode」（bus 動畫暫停、title=`loading.error.title`、subtitle=`LocalizedError.message`、第 N dot 變 `--color-warn`、retry button 出現）/「Retry re-dispatches the same add_vault call」（mock `useVaultsStore.addVault` 被以原 path + mode 呼叫、state 重設為 phase 0）。實作 failure mode 行為：偵測 `useVaultsStore.error` 由 null 變非 null 進入 failure；retry 按鈕 onClick 抓上次 addVault 的 (path, mode)（透過 `useVaultsStore` 內補一個 `lastAddVaultArgs` 暫存）重 dispatch。驗證：`pnpm test LoadingOverlay.failure` 綠、CDP smoke 階段 7 故意製造失敗時 retry 真能重跑。
- [x] 6.2 在 `LoadingOverlay.test.tsx` 加「Slow phase shows dim hint」test：fake timer 推 20s 無新 event、assert `loading.slow.hint` element 出現、再來一個 event 切 phase 後 hint 消失。實作：state 增 `slowHintVisible: boolean`、phase 進入時 `setTimeout(() => setSlowHintVisible(true), 20000)`、phase 切換 clear timeout。驗證：`pnpm test LoadingOverlay.slow` 綠。

## 7. i18n keys（per design「i18n key 命名」）

- [x] 7.1 在 `codebus-app/src/i18n/messages.ts` en 區段（行 65–70 附近）與 zh 區段（行 615–616 附近）各加 9 條新 key：`loading.phase.1.title` … `loading.phase.6.title` + `loading.error.title` + `loading.error.retry` + `loading.slow.hint`。zh 內容對齊 design.md「Phase Mapping Table」+ proposal 內 i18n 表；en 對齊 v1.1-mocks.html §1.2 表。**`loading.title` / `loading.subtitle` 既有 key 與 value 不動**。驗證：`pnpm test i18n` 既有 messages snapshot test 重跑通過、`pnpm tsc` 綠、grep 確認 `loading.title` value 在 git diff 中未變動。

## 8. CDP smoke：真實前端跑完整流程（per design「Acceptance Criteria」+ Pre-apply 校準 5 雷）

- [x] 8.1 在 `codebus-app/scripts/.loading-overlay-smoke/driver.mjs`（新檔）寫 CDP driver：開 `--remote-debugging-port=9222`、`playwright-core connectOverCDP`、driver 流程 = 從 Lobby 觸發 New Vault 選新資料夾 → 截 6 phase 各一張（透過 phase advance 後立即 screenshot）→ 截 finished overlay fade-out 過程 → 切 locale en 重跑、截 6 phase en 各一張。注意 CDP WebView2 5 雷：React batching 分兩段 eval、Tailwind transition focus 後 sleep ≥500ms、CDP click retry 不誤觸鄰近卡片、Settings modal 切 locale 用 `settings-save` testid、不靠 prefers-reduced-motion CDP emulation。驗證：driver 跑完 `codebus-app/scripts/.loading-overlay-smoke/` 下有 zh + en 各 6 張 phase 截圖 + finished 截圖、SMOKE-REPORT.md 列每張截圖對應 phase 與斷言點。
- [x] 8.2 在 driver 內加 failure path 與慢階段模擬：dev build 下注入 monkey-patch 把 `addVault` IPC 改回 `Err(AppError::Internal { message: "permission denied" })` 一次、截失敗模式 UI（zh + en 各一張、含 retry button amber-warm）；另注入「phase 3 故意 sleep 21s」測 slow hint 出現截圖。驗證：driver 結束時 `.loading-overlay-smoke/` 多 4 張截圖（zh-failure / en-failure / zh-slow-hint / en-slow-hint），SMOKE-REPORT.md 註明使用的注入手段。
- [x] 8.3 在 driver 內加 fallback path 模擬：dev build 下注入「frontend 暫時 ignore `vault-init-progress` event」、跑完整 addVault、截過程中 overlay 應全程是 phase 0 fallback render（v1 `loading.title` + `loading.subtitle`、無 dots）、IPC 成功後仍 fade-out 200ms。驗證：截圖證明 fallback path 不 break；SMOKE-REPORT.md 註明這條對應 spec scenario「Backend never emits events but IPC succeeds」。

## 9. Phase timing 量測（per design「Out of Scope」例外：本 change 只收量測、follow-up 1 週後決定）

- [x] 9.1 在 driver 內加 phase timing 收集：跑 1 個完整 init（vault size 中等、~500MB source）、收集每 phase 第一個 event 到下一 phase 第一個 event 的時間差，寫進 `codebus-app/scripts/.loading-overlay-smoke/PHASE-TIMING.md` 表格（columns: phase / first event variant / duration_ms / observation）。驗證：file 存在且含 6 行 phase 資料、附 1 段觀察結論「phase 1 / phase 6 是否 < 500ms」供 follow-up 1 週後決定是否併。

## 10. 整體驗證收尾

- [x] 10.1 跑 backend 全測試：`cargo test -p codebus-core`（既有 init.rs 不退化）+ `cargo test`（workspace、含本 change 新增的 vault_progress + vault_list 測試）。驗證：兩條都全綠、warning 數量未增加。
- [x] 10.2 跑 frontend 全測試：`pnpm tsc`（VaultInitProgress TS interface 與 Rust struct 欄位名對齊）+ `pnpm test`（含 PhaseDots / LoadingOverlay 6 phase / failure mode / slow hint / fallback / QuizTab 不退化）。驗證：兩條都全綠。
- [x] 10.3 跑 `pnpm test forbidden-behaviors` 確認本 change 沒引入禁忌行為（含 ⌘K palette、telemetry call、theme toggle 等 spec「Forbidden Behaviors in v1」requirement 條目）。驗證：綠。
- [x] 10.4 對齊 spec 全部 scenario（Vault Init Progress Event 4 + LoadingOverlay Live Progress 9 = 13 scenario）並對照 design.md「In Scope」/「Out of Scope」/「Failure Modes」三段：逐條檢查 CDP smoke 截圖或自動 test 已驗、In Scope 列出的 5 區塊都有對應 task 完成、Failure Modes 描述的三條失敗路徑（IPC error / 慢階段 / event 沒到）各有對應 verification。產出 `codebus-app/scripts/.loading-overlay-smoke/SCENARIO-COVERAGE.md` 列每條 scenario 名 + 驗證手段（test 名 or screenshot 檔名）+ In Scope / Failure Modes 條目對應。驗證：所有 scenario 至少一個驗證手段對應，無遺漏；SCENARIO-COVERAGE.md 對 design.md 三段全部 cross-reference 完整。
