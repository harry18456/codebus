## Why

LoadingOverlay 在 `addVault` init-heavy 分支期間是全屏過場。v1 顯示一行靜態副標（「建立 vault 中：複製 source、掃 PII、寫 wiki 結構、建巢狀 git…」）長達 3–15 秒，user 看到 4 件事一起列出但無法判斷現在卡在哪、是不是 hang 住。Backend 的 `codebus-core/src/vault/init.rs` `run_init` 已 emit 22 個 `InitEvent`，但 Tauri 層 `vault_list.rs:207` 把 `on_event` 設成 noop `|_| {}`，資訊全丟掉。AUDIT.md `LOI-1` 已 spec lock 為 v1.1 design audit Phase 6 最後一塊；做完同時消掉 `LO-1`（副標把 PII / git internals 黑話露給 user）。

## What Changes

- Tauri 層改寫 `add_vault_at`：sync → async + accept `AppHandle`，把 InitEvent → Tauri event `vault-init-progress` emit 給 frontend；payload 規格 normalize 成 phase 編號 1..6（不直接吐 Rust enum variant 名）。
- 22 個 InitEvent 收斂成 6 phase（design v1.1 lock，per AUDIT.md line 583-610）：phase 1 準備車庫 / phase 2 複製源碼並掃過敏感資料 / phase 3 建立獨立 git 倉庫 / phase 4 搭起 wiki 結構 / phase 5 註冊到 Obsidian（可 skip）/ phase 6 上路前最後檢查。
- `LoadingOverlay.tsx` 從靜態副標升級成 6 phase state machine：listen `vault-init-progress` event、隨 phase 切換動態副標、6-dot 階段指示器（reuse Phase 5.4 `StepDots`、由 local function extract 成共用元件並擴 props）、minimum 300ms per phase（避免一閃即逝）、finished fade-out 200ms。
- 失敗模式：bus 動畫停下、step dot 變紅、標題改「車子卡住了」、副標寫 `LocalizedError`、加 retry 按鈕（amber-warm、reuse `--color-warn` token、與 02c Interrupted 同色語、不 hard-fail red）。
- 慢階段 hint：phase > 20s 無進展時副標下方加 dim hint「（這步比平常久一點，再等等…）」。
- Fallback：Tauri event 沒進來時走 v1 靜態副標 + bus 動畫路徑、不 break v1 行為。
- i18n 新增 10 條 key（`loading.phase.1..6.title` + `loading.error.title` / `loading.error.retry` + `loading.slow.hint` + reuse `loading.title` 既有 key 不改名），zh-tw + en 各一份。
- `codebus-bus-roll` keyframe 不動、phase 切換時 bus element 不 unmount（cross-phase ambient signal）。

## Non-Goals

- 不改 `codebus-core/src/vault/init.rs` `InitEvent` enum schema 本身（22 個 variant 保留原樣、本 change 是 Tauri 層 + frontend 的事）。
- 不改 `codebus-bus-roll` keyframe（v1 動畫 reuse）。
- 不動 `useVaultsStore` `initInProgress` flag 機制（既有觸發點 reuse）。
- 不改 LO-2 標題「公車正在發車…」wording（AUDIT 標 open，待 user 確認方向；wording 改另開 trivial 小 change）。
- 不獨立修 LO-1 v1 靜態副標 wording（per AUDIT「若 LOI-1 做了、LO-1 自動消失」）。
- 不做大 phase transition 動畫（subtle subtitle / dot 變化即可、`prefers-reduced-motion` 提供 instant）。
- Phase timing 量測（量到 phase 1 / phase 6 是否 < 500ms 反覆閃過）不在本 change 收斂—— AUDIT 對齊點 1 已明訂「先實機跑 1 週、若閃才收進相鄰階段」；本 change 範圍只到記錄、follow-up 另開。
- LO-3 動畫詞彙表一致性（行進 vs 怠速 vs spinner）不在本 change 範圍。
- 不重造 step dots 元件（reuse / extract Phase 5.4 `StepDots`，**不複製貼上**）。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-shell`: 新增「Vault Init Progress Event」 requirement（規範 `vault-init-progress` Tauri event payload + phase mapping），新增「LoadingOverlay Live Progress」 requirement（規範 6 phase state machine / minimum 300ms / fade-out 200ms / fallback / 失敗模式 / 20s hint）。

## Impact

- Affected specs: `app-shell`（新增兩個 requirement、不改既有 requirement）
- Affected code:
  - Modified:
    - codebus-app/src-tauri/src/ipc/vault_list.rs（`add_vault_at` sync→async + accept `AppHandle`、emit `vault-init-progress`、InitEvent→phase mapping）
    - codebus-app/src/components/LoadingOverlay.tsx（6 phase state machine、listen event、minimum 300ms timer、failure mode、fade-out 200ms、fallback path）
    - codebus-app/src/components/workspace/QuizTab.tsx（local `StepDots` function extract 為共用元件、保留 `quiz-wizard-step-dots` testid 行為相容）
    - codebus-app/src/i18n/messages.ts（en + zh 各新增 9 條 key、line 65–70 區段擴展；`loading.title` / `loading.subtitle` 既有 key value 不動）
  - New:
    - codebus-app/src/components/PhaseDots.tsx（從 QuizTab `StepDots` extract、接 `total` / `current` / `state: "running" | "done" | "error"` props，支援 4-dot 與 6-dot）
    - codebus-app/src-tauri/src/ipc/vault_progress.rs（`VaultInitProgress` event payload struct、InitEvent→phase mapping 邏輯、unit test；放 Tauri layer 而非 codebus-core 以保 core 純粹）
    - codebus-app/src/components/LoadingOverlay.test.tsx（6 phase state machine、minimum 300ms、failure mode、fallback、20s hint test）
    - codebus-app/scripts/.loading-overlay-smoke/（CDP smoke 截圖 + driver、zh + en 兩 locale、含失敗與慢階段模擬）
  - Removed: (none)
