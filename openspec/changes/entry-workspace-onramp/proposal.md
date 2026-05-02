## Why

phase7-onboarding-polish archive（2026-05-02）後 audit 暴露：**前端從未實作「entry → 新建 workspace → 跑 tutorial」的入口 UI**。後端完整（`POST /scan` / `POST /generate` / `POST /explore` / `POST /qa` 全在）但 grep 整個 `web/` 對 `/scan` / `/generate` / `/explore` 為 0 命中 — 沒任何 page 或元件呼這些 endpoint。`pages/index.vue` mount 的 `AppShell` 只是 Phase 6 step 25.5 ping smoke placeholder，使用者完成 onboarding 進到 entry page 後**不知道怎麼開始用 app**。

Phase 6 30 step 全是「assumed 已有 `workspace_id`」的 view-level UX（Trust Layer 4 站、Q&A drawer、agent console、介入點），phase7 是 polish；「第一次選資料夾 + 觸發 scan + 產 tutorial」這環沒被排進任何 phase。同時這個 gap 也鎖住 D-033 B task 12.4 (b)(c) 的 hot-swap 驗證 — 沒 workspace 就沒 LLM call 可以觀察新 binding 的效果。

關鍵 leverage 點：`workspace_id` 是 path-derived（SHA-256 of canonical lowercased POSIX path，見 `auth/service.py::workspace_id_for_path`），所以 sidecar 不需要記憶 workspace list — 給定 path 就有確定 id。第一波 onramp 不用做 sidecar workspace persistence，純前端「選資料夾 → derive id → POST /scan → SSE 進度 → POST /generate → navigate `/tutorial/<id>`」就能通整條 happy path。

## What Changes

**Initial scope（鎖定）：**

- **新加 `tauri-plugin-dialog`** — Tauri 2 官方 plugin，提供 native folder picker（沿用 phase7 加 `tauri-plugin-opener` 的同一條路：Cargo dep + capabilities + npm package）。
- **重寫 `web/app/pages/index.vue`** — 從現有 ping smoke `<AppShell>` 改成 entry shell：顯示「+ 開新 codebase」按鈕（觸發 folder picker）+ scan/generate 進度 + 進 tutorial 入口。保留既有 onboarding redirect 邏輯（`onMounted` 打 `/healthz`），不破 phase7 修好的 redirect flow。
- **新加 onramp composable + 元件** — `useWorkspaceOnramp.ts` 封裝「pick folder → 4-step sidecar pipeline (scan → kb-build → explore → generate) → navigate」狀態機（兩 click：pick + 「+ 產生 tutorial」；composable 內部自動 chain 相鄰 step，見 design Decision 5）；`<WorkspaceOnrampCard>` 顯示 phase / progress / cta；`<FolderPickerButton>` wrap dialog plugin。
- **完成 onramp 後 navigate `/tutorial/<workspace_id>`** — Phase 6 step 27 R-01 station board 已實作（archive `2026-04-29` 之 `r-01-station-board`），onramp 串好就能直接通到 station 開跑 Q&A。

**P1+ defer（不在此 change，明確排除）：**

- Recent workspace list（多 workspace 切換 / 顯示最近用過）— 需要 sidecar 端 workspace registry persistence
- Workspace metadata 顯示（name / chunk count / last scan time）— 同上
- Tutorial regeneration / re-scan 入口 — 已在 step 29 介入點實作，但其入口 UI 假設 user 已在 station 內
- Multi-active workspace（同時跑兩個 scan）— 單 active workspace 在 P0 就夠

## Non-Goals

- **不重寫 D-033 B 的 settings / onboarding / keyring 流程** — phase7 才剛 archive，不動。
- **不擴 Trust Layer 四站**（R-01 / O-01 / O-04 / O-05）— 走完 onramp navigate 過去而已。
- **不做 sidecar 端 workspace registry persistence** — `workspace_id` 是 path-derived，sidecar 不需要記住 workspace list；recent list UX 留 P1+。
- **不動 Module 1 Scanner / Module 5 Generator 後端 spec** — 後端 SHALL clauses 不變，只 frontend wire。
- **不做 cross-machine workspace sync / import / export**。
- **不做 `~/.codebus/workspaces.json` 之類 frontend-managed list 持久化** — 第一波只支援 single-shot onramp，每次選資料夾都重新跑（即使 workspace_id 相同也 re-scan，sidecar `/scan` 內部會處理 idempotent / cache 行為）。
- **不修 `pages/explorer/[task_id].vue`** 的進入點 — Explorer console 仍由 station 內的「跑分析」入口觸發，不從 onramp 直接進。

## Capabilities

### New Capabilities

- `workspace-onramp`: 前端 entry page 的 workspace 創建 / 觸發 scan + generate / SSE 進度顯示 / 完成後 navigate 到 tutorial 的整條 UX 串接。

### Modified Capabilities

(none — frontend-shell capability 的 SHALL clauses 不動，只是 entry page 的 page-level 內容換了 mount target；後端 folder-scanner 與 module-5-generator capabilities 的 SHALL 都不變)

## Impact

- Affected specs:
  - `openspec/specs/workspace-onramp/spec.md`（新建）
- Affected code:
  - New:
    - `web/app/composables/useWorkspaceOnramp.ts`
    - `web/app/components/workspace-onramp/WorkspaceOnrampCard.vue`
    - `web/app/components/workspace-onramp/FolderPickerButton.vue`
    - `web/app/components/workspace-onramp/OnrampProgress.vue`
    - `web/app/utils/workspace-id.ts`（前端 SHA-256 derive，與 sidecar `workspace_id_for_path` 演算法 parity）
    - `web/tests/onramp/useWorkspaceOnramp.spec.ts`
    - `web/tests/onramp/WorkspaceOnrampCard.spec.ts`
    - `web/tests/onramp/FolderPickerButton.spec.ts`
    - `web/tests/utils/workspace-id.spec.ts`
    - `tauri/src-tauri/tests/dialog_plugin_smoke.rs`（plugin 啟用 smoke）
  - Modified:
    - `web/app/pages/index.vue`（重寫，仍保留既有 onboarding redirect onMounted）
    - `web/package.json`（加 `@tauri-apps/plugin-dialog`）
    - `tauri/src-tauri/Cargo.toml`（加 `tauri-plugin-dialog = "2"`）
    - `tauri/src-tauri/src/lib.rs`（builder 鏈加 `.plugin(tauri_plugin_dialog::init())`）
    - `tauri/src-tauri/capabilities/default.json`（permissions 加 `dialog:default`）
  - Removed:
    - `web/app/components/AppShell.vue`（Phase 6 step 25.5 placeholder shell — 被新 entry page 取代；如有其他引用會於 task 階段確認再刪）
