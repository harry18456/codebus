## Context

phase7-onboarding-polish archive 後 audit 確認：後端 `POST /scan` / `POST /generate` / `POST /explore` / `POST /qa` 完整，但前端從沒接通 — 整個 web/ 對 `/scan` / `/generate` / `/explore` 的 grep 為 0 命中。Phase 6 步驟全是「進入 station 後」的 view-level UX；entry → workspace 入口從未排進任何 phase。本 change 把這條串通的 happy path 補上：選資料夾 → scan → generate → 進 tutorial。

關鍵 leverage：`auth/service.py::workspace_id_for_path` 已實作 path-derived id（SHA-256 of canonical lowercased POSIX path），所以前後端只要對同一個 path 跑同一 algorithm 就拿同一個 id，不需要 sidecar 記憶 workspace list。第一波 onramp 不做 workspace registry persistence — recent list / metadata 顯示等 UX 累積後另開 change。

新 capability `workspace-onramp` 是純前端 + Tauri shell 增量；不動既有 spec（folder-scanner / module-5-generator / authorization-audit / interactive-tutorial）。

## Goals / Non-Goals

**Goals:**

- 使用者完成 onboarding 後，在 entry page 能透過 native folder picker 選 codebase 資料夾，無需手動 type URL 或寫 curl
- 觸發 `/scan` 時前端顯示即時 progress（透過 SSE），不留使用者面對黑屏
- scan 完成後使用者一鍵觸發 `/generate`，同樣顯示 SSE progress
- generate 完成後 navigate 到 `/tutorial/<workspace_id>`，銜接 Phase 6 step 27 r-01 station board 既有 UX
- 重啟 app 後 onramp 流程跑得通（不依賴 in-memory state）

**Non-Goals:**

- Recent workspace list / 多 workspace 同時 active / metadata 顯示（chunk count / last scan time）— 全部 P1+
- Sidecar 端 workspace registry persistence — 不做（id 是 path-derived，不需要）
- Tutorial regen / re-scan 入口 — Phase 6 step 29 介入點已實作，但其入口在 station 內，本 change 不加 entry-page 級的 trigger
- Multi-active scan / generate task — 後端 task registry 已限制 single active per kind，前端 UI 假設這條 invariant 成立
- 自動 navigate `/tutorial/<id>`（generate 完不自動跳）— 顯示「進入 tutorial」按鈕讓使用者點擊
- 動 `pages/explorer/[task_id].vue` 入口 — Explorer 仍從 station 內觸發

## Decisions

### Decision 1: workspace_id 在前端 SHA-256 derive（與 sidecar parity）

`workspace_id` 是 `sha256(path.as_posix().lower())[:12]` 加 `"ws_"` prefix（見 sidecar `auth/service.py::workspace_id_for_path`）。前端 onramp 在拿到使用者選的資料夾路徑後，**自己跑同一 algorithm** 算出 id，立即顯示給使用者並用於後續 navigate `/tutorial/<id>`。

加 `web/tests/utils/workspace-id.spec.ts` 用既有 sidecar 測試 fixture path 跑 parity assertion — 確保前後端 algorithm 不會 drift。

**Why this over server-derive：** server 端 derive 需要 round-trip 才能拿 id，但 onramp UI 想在 picker 關閉那一秒就顯示 id 與資料夾名稱，且 `/scan` 是 SSE long-running task，等 task 結束才拿 id 太慢。Path-derive 是純函數，前端跑零成本。

**Trade-off：** 同一個 hash algorithm 在兩端維護。但 path canonicalization 規則簡單（POSIX form + lowercase），跨平台行為一致；parity test 守住 drift。

### Decision 2: SSE progress 用新元件 `<OnrampProgress>`，不重用 `<ProgressStrip>`

Phase 6 step 28 的 `ProgressStrip` 是 in-station experience（顯示 explorer agent 的 step-by-step bucket fill，assume workspace 已存在）。Onramp 的 progress 是 pre-station，UI context 不同：使用者期待看到「正在掃描 X 個檔案 / 已索引 Y chunks」這類 throughput-style 數字，不是 agent step 列表。

新元件 `<OnrampProgress>` 接 SSE event 拆 phase（`scanning` / `indexing` / `generating` / `done`）+ counter，不複用 `ProgressStrip` 內部的 bucket UI 模式。

### Decision 3: scan / generate 期間使用者可離開 entry page

Onramp 的 SSE task 由後端 task registry 管，前端 `useWorkspaceOnramp` composable 用 module-level singleton（同 `useQaSession` / `useIntervention` 模式）保留 task state。使用者切到 `/settings` 或 `/audit/llm` 不會中斷 task；回 entry page 仍看到進行中或已完成的 progress。

`<TopBar>` workspace chip 在 onramp 完成後反映目前的 workspace_id（用 step 29 既有 `<SwitchWorkspaceMenu>` infrastructure；本 change 只 wire active workspace state，不擴 menu UI 範圍）。

### Decision 4: generate 完成後顯示「進入 tutorial」按鈕，不自動 navigate

完成 generate 時 SSE 收到 `done` event，`<WorkspaceOnrampCard>` 切到 ready 狀態，顯示「進入 tutorial」按鈕（`<NuxtLink to="/tutorial/<id>">`）。**不**自動跳。

**Why：** UX 自主性 — 使用者可能想再跑一次 / 換資料夾 / 直接離開 app。自動跳會跨越 onramp 與 station 兩個 UX context，使用者失去掌控感。按鈕只是一個 click 的成本。

## Risks / Trade-offs

- **[Risk]** path canonicalization 在前端與 sidecar 行為 drift（例如 Windows path separator 或大小寫處理） → **Mitigation:** parity test 用一組覆蓋 Windows backslash / mixed case / posix slash 的 fixture path，前端 `workspace-id.ts` 與 sidecar `workspace_id_for_path` 各跑一次比對。
- **[Risk]** `tauri-plugin-dialog` 在 macOS / Linux 行為差異（permissions、預設目錄）→ **Mitigation:** 沿用 `tauri-plugin-opener` 的 capabilities pattern；P0 只 Windows 驗，跨平台留 D-033 B task 12.5 那條 follow-up 一起處理。
- **[Risk]** scan / generate 跑很久使用者以為 app 卡住 → **Mitigation:** SSE event 至少每 N 秒 heartbeat（後端 task registry 已支援），`<OnrampProgress>` 顯示「掃描中…N 個檔案」+ elapsed timer。
- **[Risk]** scan 失敗（檔案太多 / 權限問題 / sidecar OOM）UX 卡死 → **Mitigation:** SSE error event 顯示 error message 在 `<WorkspaceOnrampCard>`，提供「重試」按鈕（重新走 `/scan` 流程）；不嘗試自動 fallback 或 graceful degradation。
- **[Trade-off]** 不做 recent workspace list 表示常用 workspace 每次都要重新 picker 選資料夾。**Accept：** 換得 P0 scope 控制（小於 15 task），完整 list UX 留 P1+ 累積使用模式後再設計。
