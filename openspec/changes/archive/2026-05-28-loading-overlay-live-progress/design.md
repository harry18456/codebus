## Context

LoadingOverlay 是 codebus-app 在 `addVault` init-heavy 分支期間（detect 模式無 `.codebus/` 或 re_init 模式）顯示的全屏 overlay，從 v1 上線至今維持單一靜態副標。Backend `codebus-core/src/vault/init.rs` `run_init` 早就 emit 22 個 `InitEvent`（callback closure），但 Tauri seam `codebus-app/src-tauri/src/ipc/vault_list.rs` 把 `on_event` 設成 noop（兩個 `|_| {}`，`AddVaultMode::Detect` 與 `AddVaultMode::ReInit` 各一處），資訊全部丟掉。

設計來源已 lock：
- `codebus-app/design-handoff/AUDIT.md` `### 額外畫面 · LoadingOverlay` 整段（LOI-1 / LO-1 / LO-2 / LO-3 / LO-4 共五條議題）。
- `codebus-app/design-handoff/v1.1-mocks.html` §01 LoadingOverlay（1.1 mock layout / 1.2 6 phase 對應表 / 1.3 階段語意 / 1.4 異常處理 / 1.5 對齊點）。

此 change 為 v1.1 design audit Phase 6 最後一塊，做完 LOI-1 同時自動消掉 LO-1（v1 副標把實作黑話露給 user）。LO-2 / LO-3 / LO-4 不在本 change scope。

## Pre-apply 校準（grep 結果）

照例先校 AUDIT / mock / 實機差異（per `feedback_propose_v1_spec_landing_read_audit_first` + `project_quiz_fullscreen_wizard_view_term_disambiguation`）。

### 同名詞 disambiguation

| 詞 | 在本 change 的含意 |
|---|---|
| **「loading」** | v1 靜態副標 vs v1.1 live progress 版本（本 change 主動） |
| **「phase」** | LoadingOverlay 6 phase（本 change）vs Phase 5.4 Quiz wizard 4 step（同元件、不同概念） |
| **「step dots」/「phase dots」** | 元件層複用 Phase 5.4 `StepDots`；功能上是 6-dot 階段指示器；本 change extract 為共用 `PhaseDots` |
| **「InitEvent」** | Rust enum `codebus_core::vault::init::InitEvent`（22 variant、backend）vs Tauri event `vault-init-progress`（payload normalized phase #、frontend 收這個） |

### AUDIT 與實機差異（必須在 apply 階段對齊）

| AUDIT.md / mock 寫的 | 實機 enum variant | 處置 |
|---|---|---|
| `PiiConfigLoaded` | `PiiConfigLoadWarn`（init.rs line 172） | apply 階段以實機為準；phase 2 mapping 寫 `PiiConfigLoadWarn` |
| `PiiWarn` | `PiiPatternsExtraWarn`（init.rs line 175） | 同上，phase 2 寫 `PiiPatternsExtraWarn` |

### Backend 結構校準

- `codebus-core/src/vault/init.rs` `InitEvent` enum 22 個 variant（含上述兩個 PII variant 名）：`Start` / `LayoutCreated` / `SourceGitignore` / `PiiConfigLoadWarn` / `PiiPatternsExtraWarn` / `RawSyncDone` / `InternalGitignoreDone` / `NestedRepoDone` / `SchemaDone` / `ManifestSignal` / `ManifestDone` / `SkillBundlesDone` / `NavStubsDone` / `SettingsDone` / `ObsidianResult` / `ObsidianSkipped` / `CommitDone` / `StarterConfigUnavailable` / `StarterConfigDone` / `StarterConfigError` / `Finished`（加上 debug-label fn 位於同檔案，把每個 variant 對應到字串 label）。
- `run_init` 簽名：sync function，第三參數 `on_event: impl FnMut(InitEvent<'_>)`。
- Tauri seam：`vault_list.rs` 兩處呼叫 `run_init`（`Detect` + `ReInit` 分支），現在都用 noop closure。
- `add_vault_at` 簽名：sync `pub(crate) fn`，唯一非測試 caller 是 `add_vault` IPC command（既已 `async fn`，從 frontend 那邊看是 awaited），其他四個 caller 是同檔內的 unit test。

### Frontend 結構校準

- `LoadingOverlay.tsx` 結構：72px bus emoji + `codebus-bus-roll` inline style + `t("loading.title")` + `t("loading.subtitle")`（41 行）。
- 觸發機制：`App.tsx` `useVaultsStore` 的 `initInProgress` flag；`vaults.ts` 只在 mode `detect` 或 `re_init` 把 flag 設 true。
- 既有 keyframe：`codebus-app/src/styles/globals.css:39` `@keyframes codebus-bus-roll`（不動）。
- i18n key 既有：`loading.title` / `loading.subtitle`（en `messages.ts:68-70`、zh `messages.ts:615-616`）—— 既有 key value 不動（per Phase 4A G-copy-2 教訓）。
- `StepDots` **不是共用元件**：是 `QuizTab.tsx:106` local function，hard-code `current: 1 | 2 | 3 | 4`，testid `quiz-wizard-step-dots`。本 change 要 extract 成共用 `PhaseDots` 元件並擴 props 支援 6-dot 與 state（running / done / error）。
- amber-warm token 已存在：`codebus-app/src/styles/tokens.css:26` `--color-warn: #f5a623`，並透過 `--color-status-interrupted: var(--color-warn)`（Phase 6.1 land）讓失敗模式 retry button reuse。

### serde tag collision 校準

- 既有用 `#[serde(tag = "kind", rename_all = "snake_case")]` 的 struct/enum：`AppError`、`quiz.rs` 三個、`goals.rs`、`keyring.rs`、`cli_status.rs`。
- 本 change 新增 `VaultInitProgress` payload struct（不是 tag enum、是 plain struct with `phase: u8` + `init_event_kind: String` + `elapsed_ms: u64`）—— **不衝突**。

## Goals / Non-Goals

**Goals:**

- Backend InitEvent 透過新 Tauri event `vault-init-progress` 傳到 frontend、payload 已 normalize phase（不曝露 Rust enum variant 名給前端 layout 邏輯使用）。
- LoadingOverlay 改成 6 phase state machine、隨 phase 切換動態副標、最少 300ms per phase、finished fade-out 200ms 收場。
- 失敗模式（init error）有明確 UI 表現（amber-warm、reuse 02c Interrupted 同色）。
- 慢階段（> 20s 無進展）有 dim hint。
- Fallback path 保 v1 行為（backend event 沒到也不 break）。
- `StepDots` extract 成共用元件，QuizTab 與 LoadingOverlay 共用。

**Non-Goals:**

- 不改 `InitEvent` enum schema、不改 `run_init` core 簽名語意（只在 Tauri 層改 callback closure）。
- 不改 `codebus-bus-roll` keyframe、不重置 bus 動畫。
- 不動 `useVaultsStore` `initInProgress` flag 機制。
- 不收斂 LO-2（標題 wording）/ LO-3（動畫詞彙表）/ LO-4（3-15 秒 wording 量測）。
- 不做 phase 1 / phase 6「< 500ms 是否併入相鄰階段」決定（per AUDIT 對齊點 1：實機跑 1 週後決定、本 change 只收 timing 量測檔）。

## Decisions

### Tauri event 而非擴展 IPC command 回傳

InitEvent 是 stream of progress，自然對應 Tauri event 模型（push）。改 `add_vault` IPC return shape 塞 progress array 不對：(1) IPC 是 request/response，progress 是 in-flight；(2) frontend 等 IPC 回來才畫 UI 就失去 live progress 意義。**選用 Tauri event `vault-init-progress`**，frontend 在 `add_vault` IPC 呼叫前 register listener、IPC 回來 / error 後 unlisten。

Alternative considered：用 Tauri command return iterator / channel API（experimental）—— 拒因為 codebus-app 既有 stream pattern（goal verb stream event）已是 event-based，一致性贏。

### Phase mapping 邏輯放 Tauri layer 而非 codebus-core

InitEvent variant 映射到 phase 1..6 是 UI presentation concern，不是 backend 領域概念。core 不該知道「user 看到的 6 個 step」這種分組。**放在 codebus-app/src-tauri/src/ipc/vault_progress.rs**（新檔），暴露 `fn init_event_to_phase(event: &InitEvent) -> u8` + `struct VaultInitProgress { phase, init_event_kind, elapsed_ms }`。

Alternative considered：放 codebus-core `init.rs` 新增 `pub fn phase_of(event: &InitEvent) -> u8`—— 拒因為 core 不該假設 UI 有 6 個 step、未來 CLI 端可能想要不同分組。

### add_vault_at sync to async + accept AppHandle

要 emit Tauri event 必須持有 `AppHandle`，且 emit 在 async context 比較自然。**改 `pub(crate) async fn add_vault_at(app: &AppHandle, state_path: &Path, vault_path: &Path, options: &AddVaultOptions) -> IpcResult<VaultEntry>`**。

Cascade：
- `add_vault` IPC command（既 `async fn`）—— 加參數轉發 `AppHandle`、加 `.await`。
- 四個 unit test caller—— 改用 `tauri::test::mock_app()` 取 handle、把測試包進 `tokio::test` 或 `tauri::async_runtime::block_on`；或 extract 「不需 AppHandle 的 pure 部分」獨立測試（vault list 寫入 / 路徑 normalize 等）+ 留 1-2 個 happy-path async test 驗 emit 序列。

Alternative considered：保 `add_vault_at` sync、改 `run_init` callback 用 sync channel push 到外面、上層 async loop 把 channel 訊息 emit—— 拒因為多一層 channel 沒帶來好處，純複雜化。

### Minimum 300ms per phase（所有 phase 同 strategy）

Per AUDIT 對齊點 1。實作：frontend state machine 收到新 phase event 時記下 `enteredAt`，下一個 phase 要切換時若 `now - enteredAt < 300ms` 則用 `setTimeout` 延後切；不影響 backend emit 速度，純 UI 平滑。Phase 5 Obsidian skip 是這策略的最常觸發 case（backend 馬上跳到 6）。

Alternative considered：只給 phase 5 加 minimum 300ms—— 拒因為 phase 1 / phase 6 可能也閃過、單獨處理 phase 5 等於先承認 mapping 不均勻、reviewer 角度 inconsistent。

### Failure mode 用 amber-warm 而非 hard-fail red

Init 失敗大多是 disk full / permission deny / .codebus 衝突，可 retry。Hard-fail red 預設「不可恢復」的視覺壓力。**Reuse `--color-warn`（tokens.css line 26，#f5a623）**，與 02c Interrupted banner 同色（已透過 `--color-status-interrupted` link）。Retry 按鈕 dispatch 既有 `addVault` 同樣 path、不另開 IPC。

### Phase dots 元件 extract 而非複製

`QuizTab.tsx:106` 的 `StepDots` 是 local function。複製出 6-dot 版本會留兩份 styling、未來改一定漏。**Extract 為 codebus-app/src/components/PhaseDots.tsx**，接 `total: number`、`current: number`、`state?: "running" | "done" | "error"`（default `running`）。QuizTab 改用 `<PhaseDots total={4} current={wizardStep} />`，保留 `data-testid="quiz-wizard-step-dots"` 與 `data-current-step` attribute 給既有 test。LoadingOverlay 用 `<PhaseDots total={6} current={phase} state={failed ? "error" : "running"} />`，自帶 `data-testid="loading-overlay-phase-dots"`。

### Fallback path（backend event 沒到）

Frontend state machine `phase === 0` 表示「還沒收到任何 event」。Render 路徑：
- `phase === 0`：顯示 `t("loading.title")` + `t("loading.subtitle")`（v1 既有 key、value 不動）+ bus 動畫，**不顯示 dots**（避免 6 空 dot 變視覺噪音）。
- `phase >= 1`：顯示 `t("loading.title")` + `t("loading.phase.{phase}.title")` + 6 dots。
- 失敗模式：`t("loading.error.title")` + `LocalizedError.message` + dots 凍結在當前 phase 並把當前 dot 標紅 + retry button。

如果 backend 在新版上線後突然不 emit（regression），UI 自動退到 fallback、不 white-screen。

### i18n key 命名

新 key 用 `loading.phase.1.title` … `loading.phase.6.title`（與既有 `loading.title` / `loading.subtitle` 平輩）。`loading.subtitle` 既有 key 不動（fallback path reuse），不 reuse 為 phase 1 文案（兩者語意不同：fallback 是「總覽」，phase 1 是「準備車庫」）。

## Implementation Contract

### Observable Behavior

User 按 `+ New Vault` 選一個無 `.codebus/` 的資料夾：
1. LoadingOverlay 立即出現、bus 動畫開跑、標題「公車正在發車…」、副標一開始為 fallback v1 字串（phase 0、最多 1 frame）。
2. Backend 開始 emit InitEvent；frontend 收到第一個 `vault-init-progress` event（phase 1）→ 副標換成「準備車庫」、第 1 個 dot 點亮。
3. 隨 InitEvent 一路換到 phase 6；中間每 phase 至少停留 300ms（即使 backend 比 300ms 快）。
4. 收到 `Finished` event（phase 6 的最後一個 InitEvent）→ overlay fade-out 200ms 後 unmount，主畫面切到 Workspace。
5. 失敗時 bus 動畫停止、當前 dot 紅、標題「車子卡住了」、副標顯示 `LocalizedError`、retry button（amber-warm）出現；按 retry 重跑 `addVault` 同 path。
6. 任一 phase 超過 20s 無進展 → 副標下方加 dim hint。
7. 切 locale en：全部副標 / 標題 / hint / error / retry 文案 en 化。

### Tauri Event Contract

Event name：`vault-init-progress`

Payload Rust struct（serde rename_all snake_case 與既有 IPC 一致）：

- `phase: u8`，值域 1..=6。
- `init_event_kind: String`，來自 InitEvent debug label（如 `"Start"` / `"LayoutCreated"`）。
- `elapsed_ms: u64`，從 `add_vault_at` 開始計時。

TypeScript shape（codebus-app/src/lib/ipc.ts 對應）：interface `VaultInitProgress` 對齊 Rust struct，欄位名 snake_case。

### Phase Mapping Table（authoritative，實機 variant 名為準）

| Phase | zh-tw | en | InitEvent variant |
|---|---|---|---|
| 1 | 準備車庫 | Preparing garage | `Start` · `LayoutCreated` · `SourceGitignore` |
| 2 | 複製源碼並掃過敏感資料 | Copying source · scrubbing secrets | `PiiConfigLoadWarn` · `PiiPatternsExtraWarn` · `RawSyncDone` |
| 3 | 建立獨立 git 倉庫 | Setting up isolated git | `InternalGitignoreDone` · `NestedRepoDone` |
| 4 | 搭起 wiki 結構 | Building wiki structure | `SchemaDone` · `ManifestSignal` · `ManifestDone` · `SkillBundlesDone` · `NavStubsDone` · `SettingsDone` |
| 5 | 註冊到 Obsidian | Registering with Obsidian | `ObsidianResult` · `ObsidianSkipped` |
| 6 | 上路前最後檢查 | Final checks | `StarterConfigUnavailable` · `StarterConfigDone` · `StarterConfigError` · `CommitDone` · `Finished` |

### Failure Modes

- Backend emit error → `add_vault` IPC 回 `Err(AppError::...)`、frontend 進失敗模式（不關 overlay、改顯示 retry）。
- 慢階段（單一 phase > 20s）→ 不視為失敗、只加 dim hint。
- Backend 沒 emit 任何 event 就直接回成功 → fallback path 已顯示 v1 內容，UI 不 break、直接 fade-out。

### Acceptance Criteria

- `cargo test -p codebus-core` 綠（init.rs 既有 test 不退化）。
- `cargo test`（workspace）綠：含 `vault_progress` 新模組的 serde round-trip test + `init_event_to_phase` 6-phase mapping test（每 variant 至少一個 case、用 match 強制窮舉）。
- `pnpm tsc` 綠（VaultInitProgress TS interface 與 Rust struct 對齊 snake_case 欄位名）。
- `pnpm test` 綠：`LoadingOverlay.test.tsx` 涵蓋 6 phase 切換、minimum 300ms timer、failure mode、fallback path、20s hint、fade-out 200ms。
- `QuizTab.test.tsx` 既有 `data-testid="quiz-wizard-step-dots"` assert 不退化（PhaseDots extract 後保留 testid 與 `data-current-step` attribute）。
- 真實 CDP smoke（zh + en、注意 CDP WebView2 5 雷）：新增 vault 完整流程截圖（6 phase 各一張 + finished + 失敗模擬 + 慢階段模擬 + fallback 模擬 + en locale），存 codebus-app/scripts/.loading-overlay-smoke/。
- Phase timing 量測：實機跑至少 1 個完整 init（vault size 中等），記錄每 phase 持續時間到 codebus-app/scripts/.loading-overlay-smoke/PHASE-TIMING.md，供 follow-up 評估 phase 1 / phase 6 是否要併（不在本 change 收斂）。

### In Scope

- Tauri 層 callback 改 emit event、`add_vault_at` async 化、phase mapping 邏輯與單元 test。
- LoadingOverlay 6 phase state machine、minimum 300ms、failure mode、fallback、fade-out。
- PhaseDots extract + QuizTab 改用。
- 10 條 i18n key（zh + en）。
- CDP smoke + timing 量測產出物。

### Out of Scope

- 改 InitEvent enum schema、改 codebus-bus-roll keyframe、改 LO-2 wording、收斂 LO-3 / LO-4。
- 決定 phase 1 / phase 6 是否併入相鄰階段（follow-up 1 週後）。
- 動到 codebus-core 既有 test（既有 init.rs test 不修）。

## Risks / Trade-offs

- **add_vault_at async cascade 比預期廣** → 四個 unit test caller 都要改、testing pattern 可能不一致。Mitigation：先 grep cascade range（已做、只有 4 個 test）、提供統一 helper `async fn run_add_vault_at_in_test(app: &AppHandle, ...)`，4 個 test 都用。若某個 test 純粹測 vault list 寫入、不需 init，extract「不經 init 的 pure path」function 給該 test。
- **InitEvent 變更或新增 variant，phase mapping 漏 case** → `init_event_to_phase` 用 match arm（不是 catch-all `_ =>`），compile-time 強制窮舉；未來新加 variant Rust compiler 報錯，逼開發者更新 mapping。
- **Phase 5 Obsidian skip 太快 backend 把 phase 6 多個 event 一起塞** → UI 緩衝 300ms 是針對「進入新 phase」timestamp，不是 event delay；backend 連續 emit phase 5/6 兩個 event 進來，UI 已記 phase 5 entered，phase 6 切換時檢查 `now - phase5EnteredAt < 300ms` 則延後切；不丟資料、只延遲 UI 更新。
- **Fade-out 200ms 期間 user 點到底下的 Workspace** → fade-out 期間 overlay 保留 `pointer-events: auto` 或 mount 一層 invisible blocker；簡單 css transition opacity 200ms 加期間 `inert` attribute（或保留 z-50 + opacity > 0 時點擊吃掉）即可。
- **CDP smoke 故意製造 init 失敗** → disk full 不好造、permission deny Windows 可用 readonly 父資料夾 + child 寫入；alternative：CDP driver 在 dev build 下 monkey-patch `addVault` IPC 回 `Err`、frontend 驗失敗 UI 路徑。實作時兩種都試、能簡則簡。
- **既有 `LoadingOverlay.test.tsx` 不存在** → grep 確認過、本 change 同時新建這個 test 檔（在 proposal Impact 的 New files 列）。

## Migration Plan

不需要正式 migration。新版上線後：
- 既有 user 不會感覺到差異—— init-heavy 是新加 vault 才跑、不影響既有 vault。
- 若 backend emit 在 release 後發現 regression，frontend fallback path 自動退回 v1 行為（看 v1 副標 + bus 動畫）、不 white-screen。
- 不需要 feature flag、不需要 staged rollout。

## Open Questions

無未決問題。AUDIT 對齊點 1（phase 1 / phase 6 timing）已決議「實機跑 1 週後決定」、不在本 change 收。
