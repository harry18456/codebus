## 1. 前置驗證

- [x] 1.1 確認 baseline：`cd web && npm run typecheck` 全綠 + `npm run dev` HTTP 200（既有 Nuxt 4.4.2 + landing page 起點正常）
- [x] 1.2 讀 `design/v1/README.md` §三 Phase A 13 項清單 + §四 mockup vs 實作差異 + 紅線；讀 `design/v1/tokens.css` / `shell.css` / `shell.js` 三份共用骨架原件

## 2. Tailwind 設定（Design tokens originate from a single source）

- [x] 2.1 新建 `web/tailwind.config.ts`：把 `design/v1/tokens.css` 的 surface（`--bg` / `--panel` / `--panel-2..4` / `--border` / `--border-soft`）port 成 `theme.extend.colors.surface.{0..4}` + `colors.border.{base,soft}`
- [x] 2.2 在同一份 config 補 `colors.text.{base,dim,mute}`（對應 `--text` / `--text-dim` / `--text-mute`）
- [x] 2.3 在同一份 config 補七個 oklch accent token（`accent` / `accent-2` / `green` / `yellow` / `orange` / `red` / `purple`）字面量必須與 `design/v1/tokens.css` 對應 `--*` 變數逐字相符
- [x] 2.4 在同一份 config 補 `fontFamily.sans` `[Inter, "Noto Sans TC", system-ui, sans-serif]` + `fontFamily.mono` `[JetBrains Mono, ui-monospace, Menlo, monospace]`
- [x] 2.5 確認 Google Fonts 的 Inter / Noto Sans TC / JetBrains Mono 引入路徑（透過 `web/app/app.vue` `useHead` 或 `nuxt.config.ts` `app.head.link`），對齊 `design/v1/01-home.html` `<link>` 用法

## 3. Layout 三段式（design tokens 套用）

- [x] 3.1 新建 `web/app/layouts/default.vue`：實作 `.cb-app` / `.cb-split` / `.cb-stage` / `.cb-audit` 三段 grid（1fr 360px），對齊 `design/v1/shell.css` 的 layout primitives
- [x] 3.2 layout 內 slot 結構規劃：`#topbar` / `#stage` / `#audit`，預留給 TopBar + page 主要內容 + AuditPanel
- [x] 3.3 [P] layout 全部用 design token utility class（`bg-surface-0` / `text-text-base`），不可出現 raw hex 或 Tailwind 內建 palette（`bg-slate-*` / `bg-indigo-*` / `bg-zinc-*` 全禁）

## 4. TopBar component

- [x] 4.1 [P] 新建 `web/app/components/layout/TopBar.vue`：port `design/v1/shell.js::CB_TOPBAR` helper 為 Vue component；props `workspace: string` / `task?: string` / `tab?: 'learn' | 'reasoning' | 'audit'` / `kill: 'READY' | 'ARMED' | 'OFF'`
- [x] 4.2 TopBar slot 結構：brand（🚌 + CodeBus）/ workspace switcher button / 3 tab（Learn / Reasoning / Audit）/ session widget（model + tokens + cost）/ settings icon / Kill switch
- [x] 4.3 TopBar 全部用 design token，禁 raw hex / Tailwind built-in palette

## 5. AuditPanel — 七 tab 對應七層 audit JSONL（AuditPanel surfaces seven workspace-level audit JSONL tabs）

- [x] 5.1 [P] 新建 `web/app/components/audit/AuditPanel.vue`：port `design/v1/shell.js::CB_AUDIT_RAIL` + `CB_mountAudit` 整合
- [x] 5.2 props 設計：`activeTab: 'sanitize' | 'tool' | 'reasoning' | 'token' | 'llm' | 'kb_growth' | 'generator'`（TypeScript literal union 強制 7 值），`counts?: Record<TabKey, number>`，`rows: AuditRow[]`（empty array 預設）
- [x] 5.3 七 tab 順序 left-to-right 必須是 sanitize → tool → reasoning → token → llm → kb_growth → generator（對應 CLAUDE.md 「七層 Audit JSONL」段次序）
- [x] 5.4 empty state：`rows.length === 0` 時顯示 documented empty-state message（不渲染任何 sample row）
- [x] 5.5 production 程式碼禁出現 `CB_AUDIT_SAMPLES` 字面量（grep `web/app/` 應 0 命中）

## 6. useSidecar composable —（Sidecar bearer and base URL come from Tauri IPC）

- [x] 6.1 [P] 新建 `web/app/composables/useSidecar.ts`：暴露 `useSidecar()` 回 `{ bearer: Ref<string>, baseUrl: Ref<string>, ready: Ref<boolean>, fetch: typeof fetch }`
- [x] 6.2 bearer + baseUrl 透過 `@tauri-apps/api/core::invoke('sidecar_handshake')`（或既有對應 IPC command）取得；ref values 在 IPC resolve 前為空字串、`ready=false`，resolve 後 `ready=true`
- [x] 6.3 `fetch` wrapper 自動注入 `Authorization: Bearer ${bearer.value}` header；URL 若以 `/` 起頭則 prepend `baseUrl.value`
- [x] 6.4 bearer / port / baseUrl 必須只在 useSidecar.ts 內存（in-memory ref）；禁寫進 `localStorage` / `sessionStorage` / `IndexedDB` / `document.cookie`
- [x] 6.5 grep 整 `web/app/` 對 `Bearer\s+[A-Za-z0-9_\-]{16,}` / 32+ 字 hex 字串 / `localhost:\d{4,5}` 三 pattern 應 0 命中

## 7. useSseTask composable（useSseTask consumes bearer through useSidecar）

- [x] 7.1 [P] 新建 `web/app/composables/useSseTask.ts`：function signature **必須是** `useSseTask(taskId: string)`，**禁** 接 `bearer` / `token` / `baseUrl` / `headers` 或等價值為參數
- [x] 7.2 內部呼叫 `useSidecar()` 拿 bearer，連接 `${baseUrl}/tasks/${taskId}/events` SSE endpoint
- [x] 7.3 `taskId` validate 用 regex `^(scan|kb|explore|generate|qa)_[0-9a-f]{8}$`；invalid 不開 EventSource，`status='error'` + `error.value` 引述 regex
- [x] 7.4 reactive 回傳 `{ events: Ref<SseEvent[]>, status: Ref<'connecting'|'open'|'reconnecting'|'closed'|'error'>, error: Ref<Error | null>, close: () => void }`
- [x] 7.5 reconnect exponential backoff：1s → 2s → 4s → 8s → 16s → 30s（cap），`status` 在 reconnect 期間為 `'reconnecting'`，重連成功翻 `'open'`
- [x] 7.6 events array FIFO cap 1000：第 1001 筆觸發最舊 1 筆 evict，array length 維持恰 1000

## 8. 既有檔重寫

- [x] 8.1 改寫 `web/app/components/AppShell.vue`：所有 Tailwind 預設色（`bg-slate-*` / `bg-indigo-*` / `text-slate-*`）改 design token utility（`bg-surface-0` / `text-text-base` / `bg-accent`），enforce Design tokens originate from a single source 不變式
- [x] 8.2 AppShell.vue 的 ping 邏輯抽進 `useSidecar()`：移除 `import('@tauri-apps/api/core')` 直接呼叫，改 `const { fetch, ready } = useSidecar(); await fetch('/healthz')`
- [x] 8.3 改寫 `web/app/app.vue`：從 `<AppShell />` 改 `<NuxtLayout><NuxtPage /></NuxtLayout>` 結構；既有 ping 按鈕內容移到 `web/app/pages/index.vue`（新建）
- [x] 8.4 新建 `web/app/pages/index.vue`：寄居 ping 按鈕 + Sidecar handshake smoke test 內容（從 AppShell 搬過來）；用 `<NuxtLayout name="default">` 包
- [x] 8.5 視 Nuxt 4 預設 auto-import 行為決定 `web/nuxt.config.ts` 是否需明列 `components: { dirs: [...] }` / `imports: { dirs: ['composables'] }`；預期 Nuxt 4 預設規範 `app/components/` 與 `app/composables/` 已自動掃描，typecheck + dev server 跑得起來即不必改

## 9. 整合驗收

- [x] 9.1 `cd web && npm run typecheck` 全綠（Nuxt 4 strict + `noUncheckedIndexedAccess: true` 嚴格模式下）
- [x] 9.2 `cd web && npm run dev` 起服務後 `curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/` 必須回 `200`
- [x] 9.3 manual visual 對照：開瀏覽器看 `localhost:3000/` 渲染結果與 `design/v1/01-home.html` 配色 / 字體 / layout 結構一致（surface-0 深色背景、accent 青色按鈕、Inter 字體、無 Tailwind 預設藍）
- [x] 9.4 grep enforce：
    - `rg "bg-slate-|bg-indigo-|bg-zinc-|text-slate-|text-indigo-|text-zinc-" web/app/` → 0 命中
    - `rg "CB_AUDIT_SAMPLES" web/app/` → 0 命中
    - `rg "Bearer\s+[A-Za-z0-9_\-]{16,}|localhost:\d{4,5}" web/app/` → 0 命中
- [x] 9.5 既有 Sidecar Ping 按鈕（搬到 `pages/index.vue` 後）行為等價：點擊應仍透過 Tauri IPC 呼到 sidecar 並顯示 `status` / `port`

## 10. 文件連動更新

- [x] 10.1 改 `CLAUDE.md` 子系統段 `web/` 描述：landing page 改為 shell baseline + 七 tab AuditPanel 落地 + 兩個 composable 落地
- [x] 10.2 改 `CLAUDE.md` Phase 6 動工順序段：新增「步驟 25.5（已完成 phase6-shell archive）」row 在步驟 26 之前
- [x] 10.3 改 `docs/implementation-plan.md §六` 表格：插入步驟 25.5 row 描述共用骨架（1-1.5d）+ 對應 spec frontend-shell；步驟 26-30 工期不動

## 11. 規格覆蓋錨點（apply 階段純驗證 checkbox）

- [x] 11.1 Spec coverage：`Design tokens originate from a single source` 由 task 2.1-2.5 + 3.3 + 4.3 + 8.1 + 9.4 grep 滿足
- [x] 11.2 Spec coverage：`Sidecar bearer and base URL come from Tauri IPC` 由 task 6.1-6.5 + 8.2 + 9.4 grep 滿足
- [x] 11.3 Spec coverage：`AuditPanel surfaces seven workspace-level audit JSONL tabs` 由 task 5.1-5.5 + 9.4 grep 滿足
- [x] 11.4 Spec coverage：`useSseTask consumes bearer through useSidecar` 由 task 7.1-7.6 滿足
