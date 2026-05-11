## 1. Scaffolding 與 workspace 整合

- [x] 1.1 建立 codebus-app 的 Cargo workspace member 落實 Tauri Shell Runtime 並對齊 design 「Cargo workspace 子結構：codebus-app 內含 src-tauri 子資料夾」：`codebus-app/src-tauri/` nested crate、Cargo.toml 依賴 codebus-core、註冊進 workspace root；行為驗證 = `cargo check -p codebus-app-tauri` 在 workspace root 跑得過
- [x] 1.2 [P] 建立 Vite + React 19 + TypeScript 前端骨架（codebus-app/ 根目錄）：package.json、tsconfig.json、vite.config.ts；行為驗證 = `npm install && npm run dev` 開得起 Vite dev server 並 render 一個 hello-world 元件
- [x] 1.3 設定 Tauri v2（對應 design.md 「Tauri v2 而非 Electron」）：tauri.conf.json 預設 window 1280×800、min 960×640、single-instance plugin、無 system browser chrome；行為驗證 = `cargo tauri dev` 開窗符合三個屬性，雙開 binary 第二次只 focus 既存視窗

## 2. Design token 與 shadcn primitive 初始化（Design Token Translation to Tailwind v4 Theme）

- [x] 2.1 落實 Design Token Translation to Tailwind v4 Theme：把 canonical token 翻成 Tailwind v4 `@theme` 宣告於 codebus-app/src/styles/tokens.css（對應 design 「Tailwind v4 加 shadcn/ui token 整合」）：bg/border/fg/accent/semantic colors + 三個 radii + spacing scale，值對齊 codebus-app/design-handoff/README.md「Design Tokens (canonical)」段落；行為驗證 = `npm run build` 通過 + 一個 sandbox 元件渲染 `bg-[var(--accent)]` 顯示正確的 #f5a623 amber
- [x] 2.2 [P] 初始化 shadcn/ui 並安裝五個 primitive（Button、Dialog、Input、Select、Slider）只在 dark mode；行為驗證 = sandbox 路由渲染五個 primitive，截圖比對 design-handoff Settings modal 中的對應控制元件樣式

## 3. Rust IPC 基礎（IPC Command Registry、AppError Discriminated Union、對應 design.md 「IPC error 型別」）

- [x] 3.1 落實 AppError Discriminated Union 並對齊 design 「IPC error 型別：single discriminated union」：定義 AppError enum 帶 `serde(tag = "kind", rename_all = "snake_case")` 及六個 variant（io、config_parse、vault_not_found、vault_already_exists、invalid、internal），對 frontend 序列化為 discriminated union；行為驗證 = Rust unit test 對每個 variant assert JSON shape（含 `kind` 欄位與其他必要欄位如 `path` / `field`）
- [x] 3.2 落實 IPC Command Registry：註冊 Tauri command registry 暴露**僅僅五個**指令（list_vaults、add_vault、remove_vault、load_global_config、save_global_config），不註冊任何其他 command；行為驗證 = Rust 測試讀 `tauri::generate_handler!` 列表斷言長度為 5 並只包含這五個名字
- [x] 3.3 [P] 在 codebus-app/src/lib/ipc.ts 寫 type-safe TypeScript wrapper（手寫 type，不引入 specta），對每個 command 暴露 typed function 與 AppError discriminated union type；行為驗證 = `tsc --noEmit --strict` 通過、且 frontend 呼叫 unknown command 在 type 層次就被擋

## 4. App-state.json 持久化（App-State Persistence、對應 design.md 「App-state.json schema 版本化」）

- [x] 4.1 落實 App-State Persistence：實作 app-state.json 讀寫 helper 含 schema_version: 1、vault_list array（每筆含 absolute path / display_name / last_opened ISO 8601 UTC）；行為驗證 = Rust unit test 涵蓋（a）檔案不存在 → 建立 `{ schema_version: 1, vault_list: [] }`；（b）JSON parse 失敗 → log warn + 回空 list 不覆寫；（c）schema_version > current → 同上
- [x] 4.2 確保 CLI 不讀不寫 app-state.json（spec「AppConfig Namespace Isolation」前置條件之一）：行為驗證 = 用 grep 掃 codebus-cli/ 與 codebus-core/ 不應該出現 `app-state.json` 字串，紀錄結果於 PR description

## 5. Vault list 指令（Vault List Lifecycle、對應 design.md 「Vault list source of truth: app-state.json」）

- [x] 5.1 落實 Vault List Lifecycle 並對齊 design 「Vault list source of truth：app-state.json」：實作 list_vaults 讀 app-state.json、對每筆 path 驗證 fs::exists、不存在則 mark `is_missing: true`；行為驗證 = Rust unit test 涵蓋 existing path → is_missing=false、deleted path → is_missing=true、permission denied → 視為 missing
- [x] 5.2 實作 add_vault 及兩個 detection branch：無 `.codebus/` → 呼叫 codebus-core init API → 加入 list；已有 `.codebus/` → 回傳 `AppError::Invalid { field: "mode", ... }` 提示 frontend 開選擇對話（或以另外的 options arg 區分 just_bind vs re_init）；行為驗證 = Rust unit test 對 temp dir 涵蓋 fresh init 與 already-init 兩 branch
- [x] 5.3 實作 remove_vault：從 app-state.json `vault_list` 移除 entry 但不動 fs；行為驗證 = Rust unit test 斷言 `.codebus/` 目錄在 remove 後依然存在

## 6. Config 指令與 AppConfig namespace（AppConfig Namespace Isolation、對應 design.md 「Settings modal 透過 codebus-core 既有 config loader 寫檔」）

- [x] 6.1 定義 AppConfig + AppQuizConfig（pass_threshold u8 50-100 default 80、default_length u8 3-10 default 5），組進 GlobalConfig 與 codebus-core 既有的 ClaudeCodeConfig/PiiConfig/LogConfig 並列；行為驗證 = Rust unit test 涵蓋 `app.*` 缺檔時回預設值、Invalid 值（threshold=200）回 `AppError::Invalid { field: "app.quiz.pass_threshold" }`
- [x] 6.2 實作 load_global_config + save_global_config：load 透過 codebus-core 既有 loader 組 GlobalConfig；save 採 atomic write（先寫 `~/.codebus/config.yaml.tmp` 再 `std::fs::rename`）；行為驗證 = Rust unit test 涵蓋 round-trip 不掉欄位、模擬 rename 失敗時舊檔仍完整
- [x] 6.3 [P] 驗證 CLI 忽略 `app.*` namespace：對含 `app.quiz.pass_threshold` 的 `~/.codebus/config.yaml` 跑 `codebus lint`、確認無 warning、且該 key 值未被修改；行為驗證 = 整合測試 fixture（temp HOME 加 config.yaml 加 vault）跑 CLI verb 後 diff

## 7. Frontend Zustand store（對應 design.md 「Frontend state: Zustand」）

- [x] 7.1 建 vaults store（list / add / remove / set-missing actions、async 呼 IPC、loading / error 狀態），落實 design 「Frontend state：Zustand」決策；行為驗證 = React Testing Library smoke test 用 mock IPC 觸發 add 後 list size+1、remove 後 size-1
- [x] 7.2 [P] 建 settings store（load / dirty-flag / save / reset actions、AppError dispatch UI strategy）；行為驗證 = component test 改任意欄位後 dirty=true、Save 後呼 save_global_config 且 dirty=false
- [x] 7.3 [P] 建 route store（state machine: lobby | workspace-stub、`open(vault_path)` / `back()` action）；行為驗證 = unit test 涵蓋 open 後 state=workspace-stub 帶 vault 上下文、back 後 state=lobby

## 8. Lobby UI（Lobby Two-State Rendering）

- [x] 8.1 落實 Lobby Two-State Rendering 的 populated 分支：vault 卡片（display_name、path、relative last_opened，>30 天用絕對日期）、右上 `+ New Vault` 按鈕含 ⌘N 鍵盤 hint、missing badge、右鍵 context menu（Open in file manager、Remove from list）；行為驗證 = component snapshot 比對 + 一筆 missing vault 渲染出 badge
- [x] 8.2 實作 Lobby empty state：大 🚌 emoji、locale-aware 標題（system locale `zh-*` → 「來搭第一台公車吧」否則 「Board your first bus」）、subtitle、`+ Board a new bus` 主按鈕、Quick start 3 步驟卡（Pick a repo folder / Run a goal: "搞懂這 repo 的 X" / Quiz yourself to verify）；行為驗證 = component snapshot 含預期 emoji 與標題；locale switching 用 mocked locale 涵蓋兩語
- [x] 8.3 共用底部 strip（Settings gear 左、version label 右），於 Lobby 兩態與 Workspace stub 皆顯示；行為驗證 = component test 在三態下都 query 到齒輪 + version；點齒輪在任一態都開 Settings modal

## 9. New Vault flow（New Vault Flow Detection Branches、Drag-Drop Scope Limited to Lobby、對應 design.md 「Drag-and-drop folder: 用 Tauri window event」）

- [x] 9.1 落實 New Vault Flow Detection Branches 的 picker / 鍵盤入口（`+ New Vault` button 與 ⌘N / Ctrl+N 鍵盤捷徑）匯流至同一個 detection step；行為驗證 = keyboard shortcut 在 Lobby 觸發 picker、在 Workspace stub 不觸發（單元測試對兩個 route state 各驗一次）
- [x] 9.2 落實 Drag-Drop Scope Limited to Lobby 並對齊 design 「Drag-and-drop folder：用 Tauri window event」：實作 drag-drop handler 在 Lobby window（Tauri `WindowEvent::DragDrop`），Workspace stub 不註冊；行為驗證 = 手動或 e2e 測試確認 Lobby drop 觸發 detection、Workspace stub drop 無反應、drop 多個資料夾僅取第一個（其餘忽略不報錯）
- [x] 9.3 實作 detection dialog 兩 branch：Just-Bind（預設 selected）→ 直接 add_vault；Re-initialize → inner confirm 要求 user 輸入 `delete` → 刪 `.codebus/` → fresh init；行為驗證 = component test 涵蓋三條 outcome（fresh-init / just-bind / re-init-after-typed-confirm），Cancel 任一步皆無狀態改變

## 10. Settings modal（Global Settings Modal Field Set）

- [x] 10.1 落實 Global Settings Modal Field Set 含**恰恰七個**欄位：AI Provider（read-only）、Authentication（OAuth status + Re-authenticate 連結）、Default model（goal/query/fix 三個 dropdown）、PII scanner（dropdown 含 dynamic pattern count，如 `regex_basic · 14 patterns`，數字 runtime 讀 scanner registry）、Log sink（path + Change folder 連結）、Quiz pass threshold（slider 50–100% + `%` unit）、Default quiz length（slider 3–10 + `questions` unit）；行為驗證 = component snapshot 斷言七欄位、無多餘欄位（theme / language / vault-override 三個明確 NOT in markup）
- [x] 10.2 Wire Save action：呼 save_global_config 後關 modal + Saved toast；行為驗證 = component test 涵蓋 happy path（save 成功 → modal close + toast）與 failure path（save 回 AppError::Io → modal 保持開啟 + 顯示 inline error）
- [x] 10.3 Sub-label 用詞檢查：Default model sub-label **不得**出現「override」字眼，Quiz pass threshold sub-label **不得**出現「learned」「mastered」「graduated」；行為驗證 = unit test 對 Settings 元件 render 結果做 textContent 掃描，命中禁字立刻 fail

## 11. Workspace stub 與 routing（Workspace Stub Transition）

- [x] 11.1 實作 Workspace stub view：sidebar 顯示當下 vault display_name + path、main area 顯示「Workspace coming in v3-app-workspace-goal」主訊息與 `← Back to Lobby` 控制元素；行為驗證 = component snapshot 顯示三個必要元素、明確不含 wiki tree / goal list / quiz / Cmd+K 任何元素
- [x] 11.2 Wire Lobby ↔ Workspace stub transition：點 vault 卡 → route 變 workspace-stub 且 vault 上下文設定正確、點 Back → route 變 lobby 且 vault list 重新 load；行為驗證 = e2e click 測試涵蓋 round-trip、回 Lobby 後 vault_list 仍持有剛開過的 vault

## 12. Forbidden behaviors 驗證（Forbidden Behaviors in v1）

- [x] 12.1 落實 Forbidden Behaviors in v1 的驗證：撰寫 inventory 測試對 Lobby / Settings / Workspace stub 三個 root component render 結果做 DOM scan，斷言**不出現**：theme toggle、language switcher、quest banner、Recent Pages panel、graph view 入口、Cmd+K UI element、Tutorial slideshow trigger；行為驗證 = inventory test fail-fast 命中任一禁項目即 fail
- [x] 12.2 確認 app shell 在 Lobby / Settings flow 不發出對外 network request：手動透過 Tauri devtools 的 network panel 觀察跑完 add vault → open settings → save 全流程，預期 zero outbound request；行為驗證 = 截圖紀錄附入 CHANGE_VERIFY 註記

## 13. Manual 與 cross-platform dev 驗證（design.md「Acceptance criteria」）

- [x] 13.1 Windows MSVC 主開發機跑完整 acceptance checklist：首啟 empty state、`+ Board a new bus` 選 fresh repo → 看到 vault 卡片、重啟 → 卡片仍在、drag-drop bind 既有 `.codebus/` 走 Just-Bind → 加入 list、開 Settings 改 Quiz pass threshold 80→70 → Save → 重開驗證為 70 且 `~/.codebus/config.yaml` 內含 `app.quiz.pass_threshold: 70`、點 vault 卡 → Workspace stub 顯示 → Back to Lobby 回到 Lobby；行為驗證 = 六項手動勾選紀錄於 CHANGE_VERIFY 註記
- [x] 13.2 把跨平台（macOS / Linux）acceptance 驗證的 owner 正式轉給 `v3-app-polish-ship`：(a) `docs/v3-app-roadmap.md` 加 "Cross-platform policy" 段落明文 Windows-first 政策；(b) polish-ship 的 scope 一欄擴成「含 foundation / workspace-goal / quiz-cmdk 在 mac/linux 重跑 acceptance checklist」；(c) `CHANGE_VERIFY.md` 把原本 13.2 段落改成 deferral note；行為驗證 = roadmap 內容 + tasks.md 自身 + CHANGE_VERIFY.md 三處 documentation 一致即過
