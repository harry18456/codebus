## Why

`codebus` CLI 印給使用者看的 thought 文字裡每天都會出現 `[[some-slug]]` 這類 wikilink，但目前 Rust 版完全沒做 markdown 染色（`codebus-core/src/render/renderers/terminal.rs` 第 5-7 行明寫 "Color formatting intentionally deferred"），更談不上把 wikilink 變成可以點擊跳到 Obsidian 的連結。使用者讀到 `[[buddy-cli-commands]]` 還得自己去 `.codebus/wiki/concepts/buddy-cli-commands.md` 翻檔。Legacy TypeScript 版（`legacy/ts-src/src/ui/render.ts:114`）早就有 `chalk.cyan.underline` 的 wikilink 染色，Phase 1 design 第 861 行也把「OSC 8 hyperlink for `[[wikilink]]`」列為 phase 2 工作——這次一次補完。

## What Changes

- **Render layer 補上輕 markdown 染色**：`**bold**` / `` `inline code` `` / `[[wikilink]]` 三類 marker，`use_color=false` 時維持 raw 文字（CI / NO_COLOR / fixture 測試 byte-equal 不爆）
- **`[[wikilink]]` 包 OSC 8 hyperlink** → `obsidian://open?vault=<sha256-id>&file=<type>/<slug>`，現代終端 Ctrl+Click 直接跳 Obsidian 開該頁
- **終端能力偵測**：用 `supports-hyperlinks` crate 偵測，不支援的終端只染色不包 OSC 8 escape（避免印垃圾字元）
- **`RenderOptions` 擴欄**：新增 `vault_id: Option<String>` / `slug_index: Option<Arc<SlugIndex>>` / `hyperlinks: bool`，由 goal/query/fix flow 啟動時建好注入
- **Slug → 路徑索引**：vault 啟動時掃 `.codebus/wiki/` 5 folder + 3 special + `goals/`，建 `slug → (PageType, rel-path)` map，render 時把 `[[slug]]` 解到實際 `<type>/<slug>` 路徑
- **Init 階段 auto-register Obsidian vault**：vault path 指 `.codebus/wiki/`，vault id = `SHA256(abs_path)[:16]`（穩定），寫進使用者層級的 `obsidian.json`
- **跨 OS config 路徑**：Windows `%APPDATA%/obsidian/obsidian.json`、macOS `~/Library/Application Support/obsidian/obsidian.json`、Linux `~/.config/obsidian/obsidian.json`
- **Idempotent 寫入**：寫之前掃 vaults，發現同 path 的 entry 就 reuse 既有 id（避免使用者手動加過後 codebus 又寫一次重複）
- **Race-safe skip**：偵測 Obsidian process 在跑 → 跳過寫入，印 hint 提醒使用者關閉後再跑 init 或手動加 vault；不嘗試在 Obsidian 開著時硬寫
- **Opt-out flag**：`codebus --repo X --no-obsidian-register` 跳過自動註冊

## Non-Goals

- **不做 `file://` URL 變體當 fallback**：點下去由系統 `.md` 預設 handler 處理，新手機器可能跳 VSCode / Notepad，不是 Obsidian，故事斷掉。寧願在不支援 OSC 8 的終端只顯示染色 wikilink 不能點，也不要 fallback 到不可預期的 app
- **不做 Obsidian community plugin**：plugin 化是另一條獨立的長期路線（Tauri tutorial app 之前不會走）
- **不解決 Obsidian sidebar 顯示「raw/」雜訊**：vault path 已決定指 `.codebus/wiki/`（不是 `.codebus/`），sidebar 內就是 5 folder 乾淨內容，不需要 `userIgnoreFilters`
- **不解決 vault name 撞名問題**：多個 codebus repo 註冊後 vault name 都是 `wiki`，但 OSC 8 URL 用 `vault=<sha256-id>` 變體（id 由 path hash 而來，必不撞），完全繞過 name 比對
- **不嘗試 Obsidian 跑著時 race-safe 寫入**：偵測 + skip + hint 已是最務實，硬寫風險高且收益低
- **不做 markdown 完整 rendering**（headings、bullet、code block 等）：phase 1 design §16 已評估 over-render 風險，這次只做 bold / inline code / wikilink 三種高頻 marker

## Capabilities

### New Capabilities

(無)

### Modified Capabilities

- `terminal-output`: 新增 markdown 輕染色（bold / inline code / wikilink）+ wikilink OSC 8 hyperlink wrap + 終端能力偵測 + RenderOptions 欄位擴充
- `vault-init`: 新增 Obsidian vault auto-register 行為（init 階段寫 obsidian.json，跨 OS path、idempotent reuse、race-safe skip-when-running、`--no-obsidian-register` opt-out）

## Impact

- Affected specs: `terminal-output`、`vault-init`
- Affected code:
  - New:
    - codebus-core/src/render/markdown_style.rs
    - codebus-core/src/wiki/slug_index.rs
    - codebus-core/src/obsidian/mod.rs
    - codebus-core/src/obsidian/config_path.rs
    - codebus-core/src/obsidian/registry.rs
  - Modified:
    - codebus-core/src/render/renderers/terminal.rs
    - codebus-core/src/render/event_renderer.rs
    - codebus-core/src/wiki/mod.rs
    - codebus-core/src/lib.rs
    - codebus-cli/src/commands/init.rs
    - codebus-cli/src/commands/goal.rs
    - codebus-cli/src/commands/query.rs
    - codebus-cli/src/commands/fix.rs
    - codebus-cli/src/main.rs
    - codebus-core/Cargo.toml
    - Cargo.lock
  - Removed: (無)
- Dependencies (codebus-core/Cargo.toml):
  - 新增 `supports-hyperlinks` crate（terminal capability detection）
  - 新增 `sha2` crate（vault id 計算；其實 codebus-core/src/fs/file_ops.rs 已用 sha256，可重用）
- 跨 OS：macOS/Linux 路徑只在 design.md 紙上設計，phase 1 主要驗證在 Windows（spike 已驗證 Win11 + Windows Terminal）；macOS/Linux 留 follow-up E2E 驗證
