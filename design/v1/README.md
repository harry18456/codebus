# CodeBus Design Mockups · 給 Claude Code 的實作指南

這份是 **設計 → 程式碼** 的 hand-off 文件。目標 codebase 是 `web/`（Nuxt 3 + TypeScript + Tailwind，npm，D-026）。

> ⚠️ **動工前必讀**（依序）：
> 1. 這份 README（看完整圖）
> 2. `../CLAUDE.md` §架構快照 + §關鍵不變式（sidecar 已實作哪些、哪些是紅線）
> 3. `../docs/sidecar-api.md`（endpoint schema · 真要打的）
> 4. `../openspec/specs/explorer-sse/spec.md`（SSE event 格式）
> 5. `tokens.css` + `shell.css` + `shell.js`（必須 port 進 Tailwind / layout）

---

## 一、檔案地圖（14 張 artboard 對 spec / endpoint）

`index.html` 是 design canvas，把所有 artboard 依 phase 分組可視化。**Trust Layer 四站（03 / 04 / 13 / 14）是評審會停的核心，優先實作**。

| #  | Mockup HTML | 對應 spec / 模組 | 對應 sidecar endpoint | Phase |
|----|---|---|---|---|
| 01 | `01-home.html` | 首啟動 / Tauri shell ping | `GET /healthz` | A |
| 02 | `02-folder-pick.html` | Workspace 選擇器 | (Tauri dialog) | A |
| 03 | `03-grant.html` | **O-01 Grant Modal**（authorization） | `POST /authorization/grant`（Stage 6 待落地） | **A** |
| 04 | `04-scan.html` | **R-01 Scan + Pass 1 Sanitize** + audit panel | `POST /scan?stream=true` SSE | **A** |
| 05 | `05-kb-build.html` | KB Builder + embed 進度 | `POST /kb/build` SSE | A |
| 06 | `06-task-input.html` | Workspace 主畫面 / 任務輸入 | (前端 only) | B |
| 07 | `07-explorer-react.html` | Module 4 Explorer ReAct loop 視覺化 | `POST /explore` SSE（`agent_thought` / `agent_action_result` / `judge_verdict` / `coverage_gaps` / `usage_delta` / `progress` / `budget_warning`） | B |
| 08 | `08-route-confirm.html` | Route 確認（5 站 + edit） | (前端 only · `route.json` preview) | B |
| 09 | `09-generator.html` | Module 5 Generator 進度 | `POST /generate` SSE | C |
| 10 | `10-tutorial-notion.html` | Tutorial 閱讀模式 · Notion 風 | (read `<ws>/codebus-tutorials/{task_id}/`) | C |
| 11 | `11-tutorial-slideshow.html` | Tutorial 閱讀模式 · Slideshow 風 | (同上) | C |
| 12 | `12-qa-drawer.html` | Module 8 Q&A drawer（站上 ⌘K 召喚） | `POST /qa` SSE（`rag_hits` / `kb_growth` / `qa_answer`） | D |
| 13 | `13-llm-call-inspector.html` | **O-04 LLM Call Inspector**（從 audit row 點開） | (read `<ws>/.codebus/llm_calls.jsonl`) | **A** |
| 14 | `14-sanitizer-diff.html` | **O-05 Sanitizer Diff**（pre/post 整檔對比） | (read `<ws>/.codebus/sanitize_audit.jsonl` + 原檔) | **A** |

---

## 二、共用骨架（一定要先做這個再做 page）

### `tokens.css` → `tailwind.config.ts`
- `--bg` `--panel` `--panel-2..4` `--border` `--text` `--text-dim` `--text-mute` 全 port 成 Tailwind `colors.surface.{0..4}` / `colors.text.{base,dim,mute}`
- accent 一律用 oklch（`accent` / `accent-2` / `green` / `yellow` / `orange` / `red` / `purple`）
- 字體：`Inter` (sans) + `Noto Sans TC` + `JetBrains Mono` (mono · audit panel 用)
- **紫色 = Sanitizer / privacy 專用**，禁止挪作他用
- **青色 (accent) = agent / primary**

### `shell.js` 的兩個 helper → Vue components
- `CB_TOPBAR(opts)` → `components/layout/TopBar.vue`（workspace 切換 / task id / tab 切換 / Kill switch）
- `CB_mountAudit('#audit', opts)` → `components/audit/AuditPanel.vue`（右側 360px rail · 七個 tab：sanitize / tool / reasoning / token / llm / kb_growth / generator）
- `CB_AUDIT_SAMPLES` 是 mockup 假資料，**實作時刪掉**，改打 sidecar `GET /tasks/{id}/audit?tab=...` 或讀對應 jsonl

### `shell.css` → `layouts/default.vue`
- `.cb-app` / `.cb-split`（grid: 1fr 360px）/ `.cb-stage` / `.cb-audit`
- 三段式 layout 是所有 page 共用，**不要每頁重寫**

---

## 三、Phase A 實作清單（這一輪做這個就好）

僅實作 Trust Layer 四站 + 共用骨架。其餘 page 用 stub 頁面（顯示 "Coming in Phase B"）。

```
□ tailwind.config.ts                  ← port tokens.css
□ layouts/default.vue                 ← port shell.css 三段 grid
□ components/layout/TopBar.vue        ← port CB_TOPBAR
□ components/audit/AuditPanel.vue     ← port CB_mountAudit + 七 tab + sample row
□ composables/useSidecar.ts           ← bearer + base url（從 Tauri IPC 拿，**禁 hardcode**）
□ composables/useSseTask.ts           ← EventSource wrapper · auto reconnect · 收 progress event
□ pages/index.vue                     ← 01-home
□ pages/workspace/grant.vue           ← 03-grant（O-01）· 打 POST /authorization/grant
□ pages/workspace/scan.vue            ← 04-scan + 右側 sanitize tab live tail
□ components/inspector/LlmCallInspector.vue ← 13（slide-in panel · 從 llm tab 列點開）
□ components/inspector/SanitizerDiff.vue    ← 14（整檔頁面 · 從 sanitize tab 列點開）
□ pages/_stub.vue                     ← Phase B/C/D 暫用
```

---

## 四、Mockup vs 實作差異（避免照抄假資料）

mockup 為了視覺敘事用了一些**虛構資料**，實作時要換成真 source。

| Mockup 行為 | 實作要怎麼換 |
|---|---|
| Audit panel 假 row（`CB_AUDIT_SAMPLES`） | 打 sidecar SSE 即時 push + 開頁時 tail 對應 jsonl 最後 N 行 |
| `13` 的 pre-sanitize 左欄原值 | **不存在**——除非 audit unlock + 即時 from 原檔 + 套同份 rules 推算 |
| `14` 的 pre-sanitize 左欄整檔 | 同上 · 左欄是 read 原檔（在 workspace 內，過 `ensure_in_workspace`），**不是**從 jsonl 還原 |
| `12` 的 ⌘K drawer | 真的綁全域 keymap，drawer state 走 Pinia |
| topbar 的 Kill switch | 接 `POST /tasks/{id}/cancel` · 紅色 confirm |
| 站數 / cost / token 數字 | 從 SSE `progress` / `usage_delta` event 累加 |

### 紅線（CLAUDE.md §關鍵不變式 #2 #3 #5 #6）
- ❌ 不可以把 pre-sanitize 原值落盤、寫 log、上傳
- ❌ 不可以 hardcode bearer / port —— 從 Tauri 啟動時 stdout handshake 拿
- ❌ 不可以跳過 `ensure_in_workspace` 直接讀檔
- ❌ Sanitizer placeholder 沒有反查表 —— UI 不要假裝可以還原

---

## 五、檔名與品牌約束

- 檔名一律 `kebab-case`（不變式 #7）
- 頁面 title 用 `CodeBus · <screen>`
- 所有 audit jsonl 路徑用 `<ws>/.codebus/<name>.jsonl`（path constant 集中在 `_audit_paths.py`，前端不直接寫死，由 sidecar API 回傳）

---

## 六、給 Claude Code 的建議起手式

```
1. 先 cd web && npm install
2. 讀 designs/README.md（這份）+ designs/tokens.css + designs/shell.css/.js
3. port tokens + shell layout 進 Tailwind / Vue 共用 component
4. 跑 Phase A 四站 + 兩個 inspector
5. 其餘 page 用 stub · 標 "Phase B/C/D"
6. 跑 npm run typecheck + npm run dev 驗收
```

不要從 page level 直接動工——共用骨架沒先做好，每頁都會重複實作 audit panel。
