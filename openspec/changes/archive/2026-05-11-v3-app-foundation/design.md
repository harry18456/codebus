## Context

codebus v3 CLI 主線（5 verb + sandbox + nested git + PII + lint feedback loop + run-log + stream rendering 共 10 個主線 change）2026-05-10 全部 ship。Brainstorming session（2026-05-11）+ Claude Design 6-screen prototype 完成、handoff bundle 已 import 至 `codebus-app/design-handoff/`。本 change 是 app v1 四條序列化 change 的第一條，定義骨架；後三條依賴它建立的 IPC 介面、design system foundation、vault list lifecycle、app-state.json schema。

現況：`codebus-app/` 在 `Cargo.toml` workspace 已存在但只是 placeholder（無實際 source）。`docs/2026-05-11-app-ux-flow-design.md` 是 UX 規格 source of truth；`codebus-app/design-handoff/README.md` 是視覺 + token source of truth。

## Goals / Non-Goals

**Goals:**

- `cargo tauri dev` 在 Windows MSVC / macOS / Linux 開發機可跑（不含 release build / installer）
- Lobby 兩態（populated + empty）能 render，可加 / 移除 / 點開 vault，狀態持久化到 `~/.codebus/app-state.json`
- Global Settings modal 7 個欄位都可 read + edit + 寫回 `~/.codebus/config.yaml`（透過 codebus-core 既有 loader）
- IPC bridge 對 frontend 暴露 5 個 type-safe Rust command；錯誤回傳 single discriminated union
- Tailwind v4 theme tokens 對齊 `codebus-app/design-handoff/README.md` Design Tokens 段落；amber 強調色克制使用
- shadcn/ui 初始化（dark mode only），只 import 本 change 用得到的 primitive（Button / Dialog / Input / Select / Slider）
- Workspace stub：點 vault 卡片切到 placeholder 畫面，足以驗證 lobby ↔ workspace state transition；不含實際 sidebar / wiki / goal UI

**Non-Goals:**

- Vault Workspace 真正內容 → `v3-app-workspace-goal`
- Wiki Tree / Wiki preview / Milkdown 整合 → `v3-app-workspace-goal`
- Goal flow / agent stream 可視化 → `v3-app-workspace-goal`
- Quiz flow / md 持久化 → `v3-app-quiz-cmdk`
- Cmd+K spotlight overlay → `v3-app-quiz-cmdk`
- Cross-platform release build / installer / auto-update → `v3-app-polish-ship`
- E2E test infra → `v3-app-polish-ship`
- Telemetry / crash reporting / analytics —— v1 完全不做
- Theme toggle / language switcher —— hard-coded dark mode + 偵測系統 locale（`zh-*` → 中文，否則 en）
- Vault-specific settings override —— v2，本 change 完全不暴露
- Multi-AI provider 切換 UI —— v2
- 教學 md 生成 / 投影片模式 —— v1.5
- Quest / station 抽象 / 進度條視覺 —— v2
- 從 `codebus-app/design-handoff/` 直接 copy production code —— handoff 是 reference 而非 source；token 值翻譯到 Tailwind theme，但 component 用 shadcn 重寫

## Decisions

### Tauri v2 而非 Electron

選 Tauri v2 的理由：Rust 後端可直接 link `codebus-core`（避免 spawn / IPC 額外 hop）、bundle 大小約 10 MB 對比 Electron 約 100 MB、原生 OS 互動（file picker / drag-drop / 系統 menu）成熟。Tauri v2 是 GA 版本（不是 alpha/beta）但相對年輕，鎖死版本 `2.0.x`，未來升 minor 走獨立 change 評估。

Alternatives considered：Electron（捨棄，太重）、Wails（捨棄，Go 後端不 reuse codebus-core）、純 web app（捨棄，無法 invoke 本機 file picker / drag-drop / OAuth flow）。

### Cargo workspace 子結構：codebus-app 內含 src-tauri 子資料夾

Tauri v2 慣例是 frontend code 跟 Rust backend 同 repo 但 Rust 部分在 `src-tauri/` 子資料夾（避免 frontend 跟 Rust source 互相干擾、避免 npm install 把 `target/` 看成 dependency）。Cargo workspace 把 `codebus-app/src-tauri/` 註冊成 member，而非 `codebus-app/` 本體。`codebus-app/` 根目錄放 frontend（package.json / vite.config.ts / src/）加上 design-handoff（reference）。

Alternatives considered：把 Rust 直接放 `codebus-app/src/`（捨棄，跟 frontend `src/` 衝突）、用獨立 sibling crate（捨棄，沒有 logical 容器）。

### IPC error 型別：single discriminated union

5 個 IPC command 共用一個 `AppError` enum（serde-serialized），frontend 對應 TS discriminated union。錯誤分類：`IoError` / `ConfigError` / `VaultNotFound` / `VaultAlreadyExists` / `Invalid` / `Internal`。Frontend 拿到後 dispatch toast UI 或 inline error。**不**為每個 command 設計獨立錯誤型別（避免 trait surface 過早設計）。

Alternatives considered：每 command 獨立錯誤型別（捨棄，5 個都要寫對應 TS 型別，重工）、純 `String` error（捨棄，frontend 無法分類處理）。

### Frontend state：Zustand

選 Zustand 的理由：scope 小、無 boilerplate、devtool 方便；store 切三個（vaults / settings / route），每個 store 小於 100 行。

Alternatives considered：Redux Toolkit（捨棄，boilerplate 過重）、Jotai（捨棄，atom 思考模式對此 scope 過 fine-grained）、React Context（捨棄，re-render perf 風險）。

### Tailwind v4 加 shadcn/ui token 整合

Tailwind v4 在 CSS 用 `@theme` directive 定義 token。本 change：(a) 在 `src/styles/tokens.css` 用 `@theme` 宣告 amber / bg / text / radius / spacing scale，值對應 handoff README Design Tokens 段落；(b) shadcn 命令初始化用 dark mode preset，再用 token 值客製覆寫；(c) `cb-*` utility 不從 handoff 抄，純用 Tailwind class 重寫。

Alternatives considered：直接 ship handoff 的 `styles.css`（捨棄，是 demo CSS 不是 Tailwind 體系，未來擴展困難）、忽略 handoff 自己挑色（捨棄，違背 brainstorming 鎖死的 design system 決定）。

### App-state.json schema 版本化

`~/.codebus/app-state.json` 包含 `schema_version: 1` 欄位。App 啟動讀取時若版本未來不認得（schema_version 大於 current），顯示 warning 加上用 empty list 兜底（不 crash 不覆寫）。未來 schema migration 走獨立 change 處理。

Schema (JSON)：

- top-level `schema_version` (integer, currently 1)
- top-level `vault_list` (array of objects)
- each vault entry: `path` (absolute path string), `display_name` (string), `last_opened` (ISO 8601 UTC string)

Alternatives considered：純 chronological 不帶 schema_version（捨棄，未來改不動）、用 yaml（捨棄，frontend 直接讀 JSON 較簡單）。

### Vault list source of truth：app-state.json

Lobby vault list 來自 app-state.json，**不**從 filesystem 掃 `~/code/` 之類。理由：（1）user 明確控制要哪些 vault 顯示，移除 vault 是 unbind 不是刪檔；（2）vault 可能在任意路徑，無法靠掃描找出；（3）掃描 stale path 若 `.codebus/` 已被刪會 panic，UX 差。

App 啟動時 verify 每個 path 仍存在；不存在 → 卡片標 missing badge，user 可選擇 remove from list。

Alternatives considered：filesystem 掃描派生（捨棄，前述理由）、CLI + app 共用 vault registry（捨棄，CLI 是 per-repo invocation 模型不需要 global vault list；增加 coupling）。

### Drag-and-drop folder：用 Tauri window event

Tauri v2 內建 `WindowEvent::DragDrop`，frontend listen `tauri://drag-drop` event 取得拖入的 path 清單。本 change 只在 Lobby state 接受 drop；Workspace state 不註冊 handler。Drop 多個資料夾 → 取第一個（v1 不支援批次新增 vault）。

Alternatives considered：HTML5 drag-drop API（捨棄，Tauri 環境下無法取得真實檔案路徑）、自訂 Rust plugin（捨棄，內建 event 已夠用）。

### Init orchestration 抽到 `codebus_core::vault::init::run_init`

Tauri 的 `add_vault`「fresh init」分支需要 codebus init 的完整 orchestration（layout / raw_sync / nested git / manifest / skill bundles / vault settings / optional obsidian register / auto commit / optional global config starter）。Implementation 階段確認 codebus-core 沒有單一 init API，CLI 的 `commands::init::run()` 直接內嵌全套流程。為了 CLI 與 Tauri app 共用同一份 init 行為，把 orchestration 抽進 `codebus_core::vault::init::run_init(repo, &InitOptions, on_event)`。CLI 透過 closure 收 `InitEvent` 把既有 banner / debug line emit 出來（output byte-equivalent，27 個 cli_routing 測試全綠）；Tauri 端傳 `|_| {}` 跑靜默。`InitOptions` 暴露 `no_obsidian_register`、`write_starter_config` 兩個旗標；錯誤回 `InitError`（typed enum），CLI 把 `InitError::Refused` 對應 ExitCode 2、其他對應 ExitCode 1。

Alternatives considered：在 Tauri 端複製一份 orchestration（捨棄，drift 風險）、改寫 CLI 改用 callback trait（捨棄，現有 closure 已經夠用）。

### Settings modal 透過 codebus-core 既有 config loader 寫檔

Settings 「Save」按下時，IPC 把 frontend ConfigPayload 物件丟給 Rust 端，Rust 端組成 codebus-core 的 `ClaudeCodeConfig` / `PiiConfig` / `LogConfig` 結構，序列化回 yaml 並寫 `~/.codebus/config.yaml`。**不**重做 config write 邏輯。新增的 `app.*` namespace（`app.quiz.pass_threshold`、`app.quiz.default_length`）由 app 端寫入；CLI 不讀 `app.*`。

Alternatives considered：frontend 直接寫 yaml（捨棄，需重做 serde 邏輯）、把 config write 移到 codebus-core（捨棄，已經在 core 了）。

## Implementation Contract

#### Behavior（使用者與 caller 觀察點）

1. **App 啟動**
    - 雙擊 macOS app / Win exe / Linux binary → Tauri window 開啟（1280×800 預設，可 resize，min 960×640）
    - 第一次啟動：偵測 `~/.codebus/app-state.json` 不存在 → 建立空 `{ "schema_version": 1, "vault_list": [] }` → 顯示 Lobby empty state
    - 後續啟動：讀 app-state.json → render populated Lobby（含未在的 vault 顯示 missing badge）
    - 偵測 `~/.codebus/config.yaml` 不存在 → 用 codebus-core 既有 `write_starter_config_if_missing` 寫預設值（與 CLI 邏輯一致）

2. **Lobby populated**
    - Vault 卡片列表，每卡片：display_name + path + last_opened（人類可讀相對時間，超過 30 天用絕對日期）
    - 右上 `+ New Vault` 按鈕，按下開 file picker
    - 拖資料夾進 Lobby 視窗 → 直接走「New Vault」detection step
    - 右鍵卡片 → context menu 「Open in file manager」加上「Remove from list」
    - 點卡片 → 切到 Workspace stub（暫時，下個 change 換成真內容）
    - 左下齒輪 → 開 Settings modal

3. **Lobby empty**
    - 大 🚌 emoji 加上「來搭第一台公車吧」加上 Quick start 3 步驟卡加上「+ Board a new bus」主按鈕
    - 同樣支援拖資料夾

4. **New Vault detection branches**
    - 選 / 拖的資料夾沒有 `.codebus/` → 自動 init（呼叫 codebus-core 既有 init 邏輯）→ 加入 list → 切 Workspace stub
    - 已有 `.codebus/` → 開 dialog「Just bind it to Lobby (recommended)」「Re-initialize (destructive)」
        - Just bind：加入 list，vault data 不動
        - Re-initialize：要求 user 輸入「delete」確認 → 刪舊 `.codebus/` → 重 init → 加入 list

5. **Settings modal**
    - 7 個欄位顯示、edit、save
    - Save：寫 `~/.codebus/config.yaml`（透過 codebus-core）→ 關 modal，顯示 toast「Saved」
    - Cancel / ESC：discard，關 modal
    - Modal 跨 Lobby 與 Workspace 都可開（左下齒輪在兩態都顯示）

#### Interface / data shape

**Tauri IPC commands**（type-safe via specta 或手寫 type binding；本 change 採手寫以減少 dependency）：

| Command | Rust signature (pseudo) | TS type | 行為 |
|---|---|---|---|
| `list_vaults` | `() -> Result<Vec<VaultEntry>, AppError>` | `() => Promise<VaultEntry[]>` | 讀 app-state.json 加上對每個 path verify 存在；missing 標 `is_missing: true` |
| `add_vault` | `(path: PathBuf, options: AddOptions) -> Result<VaultEntry, AppError>` | `(path, options) => Promise<VaultEntry>` | 偵測 `.codebus/`，依 options.mode 決定 just_bind / re_init / fresh_init；更新 app-state.json |
| `remove_vault` | `(path: PathBuf) -> Result<(), AppError>` | `(path) => Promise<void>` | 從 app-state.json `vault_list` 移除（不刪資料夾） |
| `load_global_config` | `() -> Result<GlobalConfig, AppError>` | `() => Promise<GlobalConfig>` | 透過 codebus-core 既有 loader 組合 ClaudeCodeConfig 加上 PiiConfig 加上 LogConfig 加上新 AppConfig (app.*)；缺檔回預設 |
| `save_global_config` | `(config: GlobalConfig) -> Result<(), AppError>` | `(config) => Promise<void>` | 序列化回 yaml 寫 `~/.codebus/config.yaml`（atomic write: 先寫 tmp 再 rename） |

**`AppError` 變體（serde-tagged「kind」）：**

- `Io { message }` — 檔案系統錯誤
- `ConfigParse { message }` — yaml/json parse 失敗
- `VaultNotFound { path }` — vault 在 list 但 path 不存在或拒絕存取
- `VaultAlreadyExists { path }` — add_vault 對已在 list 的 path
- `Invalid { field, message }` — Settings 欄位驗證失敗（threshold 超出 50-100 之類）
- `Internal { message }` — 其他無法預期錯誤

Serde tag = `"kind"`，rename_all = snake_case。

**`GlobalConfig` shape（frontend 加上 Rust 共用，TS 端透過手寫 type 對應）：**

- `claude_code: ClaudeCodeConfig`（從 codebus-core）
- `pii: PiiConfig`（從 codebus-core）
- `log: LogConfig`（從 codebus-core）
- `app: AppConfig`（本 change 新增）
  - `app.quiz.pass_threshold`: u8, 50-100, default 80
  - `app.quiz.default_length`: u8, 3-10, default 5

**`~/.codebus/app-state.json` schema：** 見 Decisions「App-state.json schema 版本化」段落。

#### Failure modes

- **IPC error** → frontend 收到 AppError 後依 kind dispatch：`VaultNotFound` / `VaultAlreadyExists` 顯示 inline dialog；`Io` / `ConfigParse` / `Internal` 顯示 toast；`Invalid` 顯示 field-level 紅字
- **app-state.json corrupt（parse fail）** → log error 加上用 empty list 開 app（不覆寫，留 user 手動修正）
- **app-state.json schema_version 大於 current** → 同上，warn 加上 empty list 兜底
- **vault path 不存在（user 移動或刪了資料夾）** → 卡片標 missing badge 加上 disabled hover；可右鍵 remove from list
- **config.yaml 寫入失敗（permission / disk full）** → Save 不關 modal，顯示 error；user 可重試
- **Drop 多個資料夾** → 只取第一個，不報錯（v1 限制）
- **`.codebus/` 偵測失敗（permission）** → 視為「沒有 .codebus/」走 fresh init 路線

#### Acceptance criteria

- `cargo tauri dev` 在 Windows MSVC 主開發機跑起來、能 hot-reload frontend、能 invoke 5 個 IPC command
- 手動驗證：
    - 第一次開 app → Empty state 顯示
    - 點「+ Board a new bus」→ 開 file picker → 選一個沒 `.codebus/` 的 repo → 看到該 vault 在 list 出現
    - 重啟 app → 該 vault 仍在 list（持久化驗證）
    - 拖另一個資料夾進視窗 → 偵測 `.codebus/` 存在 → 對話跳出 → 選 Just bind → 看到加入 list
    - 點齒輪 → Settings 開啟 → 改 Quiz pass threshold 從 80 到 70 → Save → 關 modal → 重開 Settings 確認值仍是 70 → `~/.codebus/config.yaml` 內容含 `app.quiz.pass_threshold: 70`
    - 點任一 vault 卡 → 切到 Workspace stub 畫面（顯示「Workspace coming in v3-app-workspace-goal」）→ 點 stub 上的「← Back to Lobby」回 Lobby
- 自動 test：
    - `cargo test -p codebus-app-tauri` 通過所有 unit test（IPC command logic、app-state.json serde、AppConfig 預設值）
    - `npm test --prefix codebus-app` 前端 component snapshot 通過

#### Scope boundaries

**In scope:**

- Tauri shell 加上 window 設定加上 IPC registry
- 5 個 IPC command 全部實作
- Lobby populated 加上 empty 兩態加上 Workspace stub
- New Vault flow（file picker 加上 drag-drop 加上兩 branch）
- Settings modal 完整 7 欄位加上 atomic save
- Tailwind v4 加上 design tokens（從 handoff README）
- shadcn/ui 初始化加上本 change 用到的 5 個 primitive
- App-state.json schema 加上 persistence
- Workspace stub 畫面（提示「下條 change 內容」）

**Out of scope:**

- Vault Workspace 真內容（sidebar / wiki tree / nav 切換）
- Goal flow / agent stream
- Wiki preview / Milkdown
- Quiz / Cmd+K
- 教學 md / 投影片
- 多 AI provider / multi-PII / wizard
- Cross-platform release build / installer
- E2E test framework
- Telemetry / auto-update
- Drag-drop 在 Workspace state 接受（只 Lobby 接受）
- Vault-specific config override

## Risks / Trade-offs

- **Tauri v2 是 GA 版本但生態相對年輕**：plugin 版本可能跟 core 不同步、文件落後實作 → Pin `2.0.x`，遇 breaking 走獨立 upgrade change
- **Tailwind v4 加 shadcn/ui 都是新組合**：shadcn 對 Tailwind v4 支援可能還未穩 → 先做 0.5 天 spike 確認 shadcn init 跟 v4 theme `@theme` directive 相容；不行則 fallback Tailwind v3
- **Frontend bundle size**：shadcn 全 import 會肥 → 只 import 本 change 用到的 5 個 primitive；用 Vite tree-shake 驗證 bundle 小於 500 KB（不含 React runtime）
- **Windows path 處理**：Rust `PathBuf` 加 JSON string 邊界 → 統一在 Rust 端 normalize（forward-slash），TS 端不操作 path、純當 opaque string 用
- **app-state.json 跟 CLI 不互讀**：未來若 CLI 想看 vault list 需要新介面 → 本 change 不解，但 schema 預留 `schema_version` 讓 future change 演化
- **shadcn dark-only**：未來想加 light mode → 要重 init token，但 v1 完全 dark mode 是明確決定
- **handoff bundle 跟實做會 drift**：實做選 shadcn 加 Tailwind 後 className / 結構必然跟 handoff `.cb-*` 不同 → 接受 drift；handoff 是視覺 reference 而非 code source
- **Atomic config write**：寫 yaml 用 tmp 加 rename pattern 避免 partial write；macOS / Linux 原生支援，Windows MSRV 用 `std::fs::rename` 也 atomic
- **IPC command 命名沒走 specta auto-gen**：手寫 TS 型別 → 跟 Rust drift 風險 → 本 change spec 鎖死命名加 signature；後續 change 若增多再考慮加 specta
