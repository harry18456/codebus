## 1. 依賴與基礎模組（可並行）

- [x] [P] 1.1 在 codebus-core/Cargo.toml 加 `supports-hyperlinks = "3"`（選 v3 以支援 Windows Terminal / iTerm2 / VSCode integrated / GNOME Terminal / Kitty / WezTerm 等主流終端 OSC 8 偵測）；不需要新增 sha2，因為 codebus-core/src/fs/file_ops.rs 已用 sha2 0.10。執行 `cargo build --workspace` 確認 dep resolve

- [x] [P] 1.2 新建 codebus-core/src/render/markdown_style.rs：實作三個 marker 替換的純函式 `style_thought_text(text, opts) -> String`：`**bold**` → ANSI bold（`\x1b[1m...\x1b[22m`）、`` `inline` `` → cyan（`\x1b[36m...\x1b[39m`）、`[[wikilink]]` → cyan + underline；當 `opts.use_color == false` 時回傳原 text 不動。再實作 `wrap_osc8(uri, text) -> String` helper 產生 `\x1b]8;;<URI>\x1b\\<text>\x1b]8;;\x1b\\`。先寫單元測試覆蓋 spec scenarios「Bold marker renders with ANSI bold escape」「Inline code renders cyan」「Wikilink renders cyan with underline」「use_color false produces no styling」「Tool events are not styled」（最後一條由 caller 控制 — markdown_style 純函式不關心 event kind，但測試確認傳 raw json 文字進去與 use_color=false 時等價）。實作 spec requirement「Apply markdown styling to thought text when use_color is enabled」

- [x] [P] 1.3 新建 codebus-core/src/wiki/slug_index.rs：定義 `SlugIndex` struct（內含 `HashMap<String, (PageType, PathBuf)>` 其中 PathBuf 是相對於 vault root 的 path 不含 `.md` 副檔名）；實作 `build(vault_paths: &VaultPaths) -> Result<SlugIndex>` 掃 5 folder + 3 special（overview/index/log）+ goals/，slug 取自 frontmatter 或 fallback 取 file stem；duplicate slug 走「最後寫入勝出」並 emit warning（已有 wiki/lint duplicate_slug 規則處理 actual error，這裡只是 best-effort）。先寫單元測試覆蓋「Empty vault returns empty index」「Concept page resolves to (Concept, concepts/<slug>)」「Special page overview.md resolves to (Special, overview)」「Goal page resolves to (Goal, goals/<slug>)」「Duplicate slug across folders takes last」。實作 spec requirement「RenderOptions carries vault context for hyperlink emission」中的 slug_index 部分

- [x] [P] 1.4 新建 codebus-core/src/obsidian/config_path.rs：實作 `resolve_obsidian_config_dir() -> Option<PathBuf>` 用 `dirs::config_dir()` + `obsidian/` 子路徑（macOS 走 `~/Library/Application Support/obsidian/`、Linux 走 `~/.config/obsidian/`、Windows 走 `%APPDATA%\obsidian\` — `dirs::config_dir()` 已正確處理）；當 `dirs::config_dir()` 回 None 時回 None；當解出的 path 不存在時也回 None（信號「Obsidian 未安裝」）。實作 `obsidian_json_path() -> Option<PathBuf>` 在 dir 後 join `obsidian.json`。寫 cfg(target_os) 分支單元測試各驗一次，含「config dir 不存在 → None」case 用 tempdir 模擬。實作 spec requirement「Auto-register .codebus/wiki/ as Obsidian vault on init」中的「跨 OS 路徑解析」與「Obsidian not installed silently skips」scenario 的偵測部分

- [x] [P] 1.5 新建 codebus-core/src/obsidian/process_detect.rs：實作 `is_obsidian_running() -> bool` 跨 OS 偵測（Windows: enumerate processes via `sysinfo` crate match `obsidian.exe`；macOS: 同 sysinfo match `Obsidian`；Linux: 同 sysinfo match `obsidian`）；若 sysinfo 已是 codebus 依賴就直接用，沒就加（檢查 Cargo.lock）。先寫測試覆蓋「Process list 不含 obsidian 回 false」cases — 不要嘗試在測試環境啟動 Obsidian，只驗 fn 簽名 + 依賴 process iteration 行得通（mock 測試以 sysinfo 自身 unit test 為信任邊界）。實作 spec requirement「Obsidian running emits hint and skips」scenario 的偵測部分

## 2. Obsidian 註冊核心邏輯

- [x] 2.1 新建 codebus-core/src/obsidian/registry.rs：定義 `VaultEntry { path: PathBuf, ts: u64, open: bool }` 與 `ObsidianConfig { vaults: HashMap<String, VaultEntry> }`（用 serde + ordermap 保留 entry 順序若 Obsidian 在乎；HashMap 即可若不在乎 — 偏向 HashMap）；實作 `register_vault(vault_path: &Path) -> Result<RegisterOutcome>` 主入口，邏輯依序：(a) 呼叫 config_path::obsidian_json_path() 拿 Path 或回 `RegisterOutcome::ObsidianNotInstalled`；(b) 呼叫 process_detect::is_obsidian_running() 為 true 則回 `RegisterOutcome::ObsidianRunning`；(c) 讀 obsidian.json 為空則初始化 `{vaults: {}}`、parse 失敗則 backup 後回 `RegisterOutcome::IoError`；(d) 計算 `target_id = SHA-256(abs_path.to_lowercase())[:16]`；(e) 掃 existing vaults 對 path normalize（Win 大小寫不敏感、其他平台敏感）找 same-path entry，找到就 reuse 該 id 只更新 ts（記為 effective_id）、找不到就用 target_id 寫新 entry；(f) 寫回 obsidian.json（UTF-8 no BOM）；(g) 回 `RegisterOutcome::Registered { vault_id: effective_id }`。先寫測試覆蓋全部 spec scenarios：「Fresh init writes new vault entry」「Obsidian not installed silently skips」（mock config_path 回 None）「Obsidian running emits hint and skips」（mock process_detect 回 true）「Existing same-path entry reuses its id」「I/O error during write logs warning and continues」（mock 寫入回 Err）。實作 spec requirements「Auto-register .codebus/wiki/ as Obsidian vault on init」與「Resolve effective vault id for hyperlink emission」

- [x] 2.2 新建 codebus-core/src/obsidian/mod.rs 匯出 `RegisterOutcome` enum、`register_vault`、`config_path` / `process_detect` / `registry` submodules；在 codebus-core/src/lib.rs 加 `pub mod obsidian;`。執行 `cargo check --workspace` 確認模組樹接通

## 3. Render layer 整合

- [x] 3.1 改寫 codebus-core/src/render/renderers/terminal.rs：擴 `RenderOptions` struct 加 `vault_id: Option<String>`、`slug_index: Option<Arc<SlugIndex>>`、`hyperlinks: bool`（default true）三個欄位（注意 RenderOptions 目前是 Copy + Default，加 Arc 後拿掉 Copy，改成 Clone；下游 `format_event` / `format_banner` 簽名從 `opts: RenderOptions` 改 `opts: &RenderOptions` 對應調整）；在 `format_event` 的 `Thought` arm 呼叫 markdown_style 系列 helper：先用 supports-hyperlinks 偵測 + `opts.hyperlinks && opts.vault_id.is_some() && opts.slug_index.is_some()` 三聯為「emit OSC 8」開關，emit 時對每個 `[[slug]]` 由 slug_index 解到 `<type>/<slug>`、組 `obsidian://open?vault=<id>&file=<path>`、wrap_osc8 包樣式後文字。先寫單元測試覆蓋 spec scenarios「Supported terminal with resolvable slug emits OSC 8 hyperlink」「Unsupported terminal renders styling only」「Slug not in index falls back to styling only」「use_color false suppresses both styling and hyperlink」「vault_id None disables hyperlink even when supported」「hyperlinks false overrides terminal detection」。實作 spec requirements「Wrap wikilinks with OSC 8 hyperlinks when terminal supports them」與「RenderOptions carries vault context for hyperlink emission」

- [x] 3.2 同步 codebus-core/src/render/event_renderer.rs：若 RenderOptions 從 Copy 改 Clone，trait 定義或 default impl 不受影響（trait 已用 &self），但任何透過 RenderOptions 初始化 renderer 的入口要對應從 by-value 改 borrow 或 clone — 跑 `cargo check --workspace` 找 break 點修一遍。其他 renderer（JsonLines / Tauri 若存在）只要不用新欄位就不需動。

## 4. CLI flag 與 init flow 整合

- [x] 4.1 在 codebus-cli/src/main.rs 加 `--no-obsidian-register` clap flag（`#[arg(long, default_value_t = false)]`），bool 值傳遞到 init / goal / query / fix 各 entry 的呼叫鏈。寫測試驗 flag 解析 + default false。

- [x] 4.2 在 codebus-cli/src/commands/init.rs 的 init 流程末段（5 folder 建完、PII filter 設好、lint 設好之後）加 obsidian register step：當 `no_obsidian_register == true` 直接 skip；否則呼叫 `obsidian::register_vault(<repo>/.codebus/wiki)`，根據 `RegisterOutcome` 分支：(a) `Registered { vault_id }` → 印 `💡 已將 .codebus/wiki/ 加入 Obsidian vault 列表（id: <vault_id>），重啟 Obsidian 即可看到`；(b) `ObsidianNotInstalled` → silent skip；(c) `ObsidianRunning` → 印 `💡 偵測到 Obsidian 正在執行，跳過自動註冊。請關閉 Obsidian 後重跑 codebus --repo X 或在 Obsidian 內手動加入 .codebus/wiki/ 為 vault`；(d) `IoError` → 印 warning 包錯誤但不 abort init。先寫整合測試覆蓋 spec scenarios「Fresh init writes new vault entry」「Obsidian not installed silently skips」「Obsidian running emits hint and skips」「--no-obsidian-register opt-out skips」「Existing same-path entry reuses its id」「I/O error during write logs warning and continues」（透過 mock obsidian module 邊界注入各 outcome）。實作 spec requirement「Auto-register .codebus/wiki/ as Obsidian vault on init」中的 init flow 整合部分

## 5. Run flow 注入 RenderOptions

- [x] 5.1 在 codebus-cli/src/commands/{goal,query,fix}.rs 三個 entry 進入 stream loop 之前：(a) 呼叫 slug_index::build(&vault_paths) 拿 `Arc<SlugIndex>`；(b) 呼叫 obsidian::register_vault 取得 `Option<String>` effective vault_id（skip 情境回 None）— 但 init 已經 register 過、避免重複註冊與 race，這裡改成「讀 obsidian.json 找 same-path entry 拿到既有 id」的輕量 lookup（registry 提供 `lookup_vault_id(vault_path: &Path) -> Result<Option<String>>` helper）；(c) 把 vault_id + slug_index 注入 `RenderOptions`、實例化 TerminalRenderer。fix.rs 雖然不一定 spawn agent，但若 `--fix` 也 render thought（lint feedback loop 多回合）也要走同樣注入。寫整合測試驗「goal 有 hyperlink 注入」「query 同」「fix 多回合 each iteration 拿到同 slug_index」。實作 spec requirement「Resolve effective vault id for hyperlink emission」中三個 scenarios

## 6. End-to-end OSC 8 byte 驗證測試

- [x] 6.1 在 codebus-cli/tests/ 加整合測試 `obsidian_hyperlink_e2e.rs`：跑 `codebus --repo <fixture> --no-obsidian-register --query "ignore me"` 對 captured stdout 驗 byte 序列：(a) 染色 marker 出現、(b) 因 `--no-obsidian-register` 故無 OSC 8 escape；再用 mock provider 在 obsidian.json 註冊一個 fixture vault 跑同樣 query、驗 stdout 含 `\x1b]8;;obsidian://open?vault=...&file=...\x1b\\` 完整序列。這條測試是 spike 等價物的 regression 防線

## 7. 文件更新（可並行）

- [x] [P] 7.1 README.md：移動「OSC 8 hyperlink for [[wikilink]]」相關項從 `legacy/ts-src/src/ui/render.ts:104` TODO 與 `docs/superpowers/specs/2026-05-04-codebus-v2-phase1-design.md:861` 列表轉成 README 的「已完成」段，敘述「Ctrl+Click [[wikilink]] 直接跳 Obsidian + 自動 vault 註冊 + 跨 OS」；docs/superpowers/REVIEW_LESSONS.md 加新 lesson 記載 spike 累積的兩個 finding：「Obsidian URI `vault=` 參數認 SHA-256 id（undocumented，path 變體當 fallback）」與「Obsidian process 在跑時偵測 + skip 是務實做法、race overwrite 風險真實但極窄」

## Design decision coverage

每條 design 決策對應的實作 task：

- design.md「URL scheme：vault id 變體當主路徑，path 變體當文件記載 fallback」→ tasks 3.1（OSC 8 URI 模板用 `obsidian://open?vault=<id>&file=<path>` 主路徑、design 內備註 path 變體當逃生口）
- design.md「Vault path 指 `.codebus/wiki/`，不是 `.codebus/`」→ tasks 2.1、4.2（registry 收 `.codebus/wiki/` 為 vault_path，init flow 也傳同一 path）
- design.md「Vault id 算法：`SHA256(abs_path.to_lowercase())[:16]`」→ tasks 2.1（registry 算 target_id 即此演算法）
- design.md「Idempotent 寫入：reuse same-path entry 既有 id」→ tasks 2.1（掃 existing vaults 找 same-path entry 找到就 reuse 該 id，覆蓋對應測試 scenario）
- design.md「Obsidian 在跑時：偵測 + skip + hint，不硬寫」→ tasks 1.5、2.1、4.2（process_detect 偵測、registry 回 ObsidianRunning、init flow 印 hint 跳過）
- design.md「終端能力偵測：`supports-hyperlinks` crate」→ tasks 1.1、3.1（dep 加上、terminal renderer 用 supports-hyperlinks 做三聯開關）
- design.md「Slug index 時機：run 啟動時 build once」→ tasks 1.3、5.1（slug_index::build 函式、goal/query/fix 進入 stream loop 前 build once 注入 RenderOptions）
- design.md「跨 OS Obsidian config 路徑」→ tasks 1.4（config_path::resolve_obsidian_config_dir 用 `dirs::config_dir()` 跨 OS 解析 Win/macOS/Linux）
- design.md「`--no-obsidian-register` opt-out flag」→ tasks 4.1、4.2（main.rs 加 clap flag、init.rs 看到 flag 直接 skip register step）
