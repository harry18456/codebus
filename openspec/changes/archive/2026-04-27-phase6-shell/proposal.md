## Why

`design/v1/README.md` 起手式直白警告：「不要從 page level 直接動工——共用骨架沒先做好，每頁都會重複實作 audit panel」。當前 `web/app/components/AppShell.vue` 用 Tailwind 預設色（`bg-slate-950` / `bg-indigo-500`），與 `design/v1/tokens.css` 的 `--bg: #0b0d10` / `--accent: oklch(72% 0.12 210)` 完全脫鉤。如果先做 `auth-flow`（4d）或任何 page 級 change，這些 page 會繼續用 Tailwind 預設色 + 各自重寫 audit panel，落地後必須 rework。

關聯 ADR：D-026（web toolchain npm + Nuxt，2026-04-27 升 Nuxt 4.4.2）。對應 `docs/implementation-plan.md §六` 第六階段前置步驟（既有 §六只列 step 26-30，本 change 補上「step 25.5 共用骨架」這個 implementation-plan 沒明列的依賴節點）。

## What Changes

按 `design/v1/README.md §三 Phase A 實作清單`前 6 項落地，全部在 `web/` 內：

- 新建 `web/tailwind.config.ts` — 把 `design/v1/tokens.css` 的 CSS variables 抽成 Tailwind theme extension（surface 0-4 / border / text / 7 個 oklch accent / Inter + Noto Sans TC + JetBrains Mono 字體）。紫色保留 sanitizer/privacy 專用、青色保留 agent/primary。
- 新建 `web/app/layouts/default.vue` — port `design/v1/shell.css` 三段 grid（`.cb-app` / `.cb-split` / `.cb-stage` / `.cb-audit` 1fr 360px），所有 page 共用。
- 新建 `web/app/components/layout/TopBar.vue` — port `CB_TOPBAR` helper：workspace switcher + 3 tab（Learn / Reasoning / Audit）+ session widget（model / tokens / cost）+ settings + Kill switch。
- 新建 `web/app/components/audit/AuditPanel.vue` — port `CB_AUDIT_RAIL` + `CB_mountAudit`，**七 tab 對應七層 audit JSONL**（sanitize / tool / reasoning / token / llm / kb_growth / generator）。Props-driven empty state，**`CB_AUDIT_SAMPLES` 假資料不複製進 production code**（v1 README §四紅線、CLAUDE.md 不變式 #2）。
- 新建 `web/app/composables/useSidecar.ts` — 從 Tauri IPC 取 sidecar bearer token + base URL，暴露 `useSidecar()` returning `{ bearer, baseUrl, ready, fetch }`。bearer / port **不可 hardcode**（v1 README §四紅線、CLAUDE.md 不變式 #5）。`fetch` 預設帶 `Authorization: Bearer ...`。
- 新建 `web/app/composables/useSseTask.ts` — EventSource wrapper：吃 `task_id`，連 `GET /tasks/{id}/events`，回傳 reactive `{ events, status, error, close }`。auto-reconnect 走 exponential backoff。bearer 從 `useSidecar()` 取。
- 修改 `web/app/components/AppShell.vue` — 從 Tailwind 預設色改用 design token；ping 邏輯抽進 `useSidecar()` 不直接 `import('@tauri-apps/api/core')`。
- 修改 `web/app/app.vue` — 從 `<AppShell />` 改 `<NuxtLayout><NuxtPage /></NuxtLayout>` 結構（Phase 6 後續所有 page 必要結構）。
- 修改 `web/nuxt.config.ts`（按需）— 如 layouts / components 需自動掃描路徑配置就補；目前 Nuxt 4 預設規範 `app/` 內子目錄自動偵測，可能不需動。

## Non-Goals

- **不裝 Pinia**（步驟 29 三介入點才裝）
- **不裝 `@nuxtjs/mdc`**（步驟 26 互動元件才裝）
- **不實作 Phase A 個別 page**（grant / scan / inspector 留給後續 change：`auth-flow` 落 03-grant、後續 change 落 04-scan / 13 / 14）
- **不對接真實 sidecar SSE endpoint**（`useSseTask` 是抽象 wrapper，後續 page 才實打）
- **不刪 `design/v1/shell.js::CB_AUDIT_SAMPLES`**（design 原件保留，production code 不引用即可；引用 enforcement 由各 page change 自證）
- **不寫 component 測試**（前端 test framework 在 Phase B 才裝；本 change 用 `npm run typecheck` + `npm run dev` HTTP 200 + render 視覺驗收）
- **`frontend-shell` capability spec 只規範不變式**（bearer 來源 / 七 tab 對應七層 audit / 無 `CB_AUDIT_SAMPLES` 字面量 / design tokens 唯一來源），**不複製 mockup 視覺細節**（視覺仍以 `design/v1/` 為 source of truth；`design/v0/README.md §五` 既定紀律「mockup 不是 spec」對齊）
- **不做 v0 補強**（O-01 三場景 / O-05 LOCKED-UNLOCKED state machine 留給 `auth-flow` / 後續 change）
- **拒絕 shell + auth-flow 合一個 change**（範圍跨 web 單 stack vs web/sidecar 雙 stack + 新 capability spec，混在同 change 風險不對稱）
- **拒絕跳過 shell 直接做 auth-flow**（v1 README §六明文警告 page 級重複實作 audit panel，當前 AppShell 已用 Tailwind 預設色脫鉤 token，會立刻 rework）

## Capabilities

### New Capabilities

- `frontend-shell`: web/ 前端共用骨架的不變式集合 — design tokens 唯一來源、Tauri IPC bearer/port 取得規則、七 tab AuditPanel 對應七層 audit JSONL、`CB_AUDIT_SAMPLES` 字面量禁止進 production code、`useSseTask` 透過 `useSidecar` 取 bearer。**只規範 invariant 與紅線，不規範視覺細節**（視覺以 `design/v1/` 為 source of truth）。

### Modified Capabilities

(none)

## Impact

- Affected specs: 1 NEW — `frontend-shell`（不變式 only：design tokens 唯一來源 / 無 hardcoded bearer / 七 tab AuditPanel / 無 `CB_AUDIT_SAMPLES` 字面量 / `useSseTask` 走 `useSidecar` 取 bearer）
- Affected code:
  - New:
    - web/tailwind.config.ts
    - web/app/layouts/default.vue
    - web/app/components/layout/TopBar.vue
    - web/app/components/audit/AuditPanel.vue
    - web/app/composables/useSidecar.ts
    - web/app/composables/useSseTask.ts
  - Modified:
    - web/app/app.vue
    - web/app/components/AppShell.vue
    - web/nuxt.config.ts（按需）
  - Removed: 無
- Affected docs:
  - CLAUDE.md（子系統段 web/ 描述更新：landing 改為 shell baseline + 七 tab AuditPanel 落地）
- Test suite delta：sidecar baseline 853 / 19 不變（純前端 change）；驗收手段為 `npm run typecheck` 全綠 + `npm run dev` HTTP 200 + manual render 對照 `design/v1/01-home.html` / `design/v1/04-scan.html`
