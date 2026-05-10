## Context

v3 CLI 沒有任何 render 抽象 — 5 個 verb 命令各自直接 `println!("✓ ...")`，無 emoji、無 color、無 clickable hyperlink。`codebus init` 印 11 行 `✓ ...` progress；`codebus lint` 印純 ASCII (`x wiki/concepts/foo.md` + `error:` / `warn: `)。

v2 的「上車舞」品牌印象（`codebus-core/src/render/renderers/terminal.rs` 的 `Banner::Start { path } => "🚌 來囉來囉~ CodeBus 駛入 ..."` 系列）對應 9 個 banner 變體 + emoji ↔ symbol fallback + color + OSC 8 hyperlink。v2 設計上有 `EventRenderer` trait + `TerminalRenderer` impl + factory pattern（為了讓 Tauri / JsonLines renderer 之後接得上）。

v3 path D 已決定：

- agent 由 claude 自帶 UI（spawn 用 `Stdio::inherit()`，agent stdout 直流到 user terminal）
- codebus 不再 render thought / tool / observation event（v2 那套是 codebus 自己 parse stream-json 才能做）
- 因此 render 抽象不需要支援 streaming event；只需要 banner（spawn 前/後）+ lint output 兩個面向

故 v3-render-polish **不**重建 v2 的 EventRenderer trait + factory（roadmap §3 anti-pattern #1：single-impl 不寫抽象）。改用 `RenderOptions` 普通 struct + 數個 free function 走「caller pass struct 進去」的 plain pattern，跟 v3-config 的 `PiiConfig` / `ClaudeCodeConfig` 同形。

v2 的 render 邏輯（5-level emoji priority、`config.yaml emoji:` 欄位、`--emoji` flag、`--no-emoji` flag、`NO_EMOJI` env）user 已明確 drop（discuss session 第 4 點：「config 改變輸出行為倒是可以先不考慮」）。剩下兩個感測層：`NO_COLOR` env（社群標準，色彩關閉）+ TTY 偵測（非 TTY 自動關 emoji+color+OSC 8，避免污染管線）。

## Goals / Non-Goals

**Goals:**

- 補 v2 `Banner` 系列 10 變體（Start / Goal / SyncStart / SyncDone / PiiSummary / LintStart / LintDone / CommitDone / Done / Hint）
- init / goal / query / fix subcommand 的 default stdout 由 banner 序列驅動（取代既有 `✓ ...` progress lines）
- lint 輸出加 emoji ↔ symbol fallback、ANSI color（`error:` 紅、`warn:` 黃）、OSC 8 hyperlink（`wiki/<path>` 行 → `obsidian://open?vault=<id>&file=<rel>`）
- `--debug` 既有 `[debug]` lines 全保留，與 banner 並存（debug = banner + 細節層）
- 環境偵測：`NO_COLOR` env、TTY 偵測（`std::io::IsTerminal`）、hyperlink 終端能力偵測（`supports-hyperlinks` crate）
- v2 `lookup_vault_id` 函式 port 進 `obsidian_register` 模組，render 用它生成 OSC 8 URL

**Non-Goals:**

- **不重建 EventRenderer trait + factory**：v3 只有一個 terminal target，無第二 impl 驗證
- **不做 stream event renderer**：v3 spawn claude 用 `Stdio::inherit()`，agent stdout passthrough；codebus 看不到 thought / tool / observation
- **不改 `Stdio::inherit()`**：banner 在 spawn 之前/之後印（codebus 自己 stdout），spawn 行為不變
- **不做 RunLog / token 追蹤**：需要改 `Stdio::piped()` parse stream-json，獨立 `v3-run-log` follow-up
- **不做 5-level emoji priority chain**：`--emoji on|off`、`--no-emoji`、`NO_EMOJI` env、`config.yaml emoji:` 全 drop
- **不在 banner 加 ANSI color**：banner 只用 emoji；color 集中在 lint issue 標籤
- **不動 lint JSON format**：machine-readable 契約已 ship，emoji / ANSI / OSC 8 永遠不可進 JSON

## Decisions

### Banner 採 enum + free function，不採 trait

```rust
// codebus-core/src/render/banner.rs
pub enum Banner<'a> {
    Start { repo_path: &'a Path },
    Goal { goal: &'a str },
    SyncStart,
    SyncDone { files: usize, mib: f64, elapsed_ms: u128 },
    PiiSummary { scanner: &'a str, scanned: usize, hits: usize, action: &'a str },
    LintStart,
    LintDone { errors: usize, warns: usize, elapsed_ms: u128 },
    CommitDone { sha7: &'a str },
    Done { wiki_path: &'a Path },
    Hint { wiki_path: &'a Path },
}

pub fn print_banner(banner: Banner<'_>, opts: &RenderOptions);
pub fn format_banner(banner: Banner<'_>, opts: &RenderOptions) -> String;
```

`format_banner` 是 pure function（給 unit test 拉 string assert）；`print_banner` thin wrapper `println!`（給 caller 用）。Enum borrow `'a` 避免 String allocation。

**Alternative considered**：trait `Renderer { fn render(&self, banner: Banner) }`。Drop — single impl 違反 anti-speculative-abstract 原則（v3 第一次嘗試被 reset 的主因之一）。

### RenderOptions 普通 struct，靜態初始化一次

```rust
// codebus-core/src/render/options.rs
pub struct RenderOptions {
    pub use_emoji: bool,
    pub use_color: bool,
    pub use_hyperlinks: bool,
    pub vault_id: Option<String>,
}

impl RenderOptions {
    /// Detect once at process start. NO_COLOR / TTY / supports-hyperlinks
    /// are checked here; not re-evaluated per banner call.
    pub fn detect() -> Self;
    /// Test seam: explicit construction for unit tests.
    pub fn explicit(use_emoji: bool, use_color: bool, use_hyperlinks: bool, vault_id: Option<String>) -> Self;
}
```

`detect()` 在 verb command 入口呼叫一次後傳給每個 banner。env 偵測規則：

- `use_emoji = is_tty()`（非 TTY 強制關，TTY 強制開 — 不接受 user override）
- `use_color = !no_color_env() && is_tty()`（`NO_COLOR` env 設定或非 TTY 都關）
- `use_hyperlinks = use_color && supports_hyperlinks::on(stdout)`（支援 OSC 8 才開）
- `vault_id` 由 caller 從 `obsidian_register::lookup_vault_id` 拿後填入（init 階段做、其他 verb 可選）

### `lookup_vault_id` port 自 v2

v2 `legacy/v2-rust/codebus-core/src/obsidian/registry.rs::lookup_vault_id` 大約 30 行：讀 `obsidian.json`、遍歷 vaults map、用 normalized abs_path 比對、回傳 `vault_id` 字串。Port 進 `codebus-core/src/vault/obsidian_register.rs`，導出 `pub fn lookup_vault_id(wiki_path: &Path) -> io::Result<Option<String>>`。

找不到時回 `Ok(None)` — render 端對 `None` 降級為「無 OSC 8，純路徑」（OSC 8 是 progressive enhancement，缺失不致命）。

### lint text 重構：分離 format 與 styling

既有 `codebus-core/src/wiki/lint/output.rs::format_text(result) -> String` signature 不破壞性改：新增重載式介面

```rust
// existing — 行為不變，內部呼叫 format_text_with_opts(&RenderOptions::no_styling())
pub fn format_text(result: &LintResult) -> String;
// new — 帶 RenderOptions、含 emoji + color + OSC 8 wrap
pub fn format_text_with_opts(result: &LintResult, opts: &RenderOptions, wiki_root: &Path) -> String;
```

`wiki_root` 是給 OSC 8 URL 計算 absolute path 用（`obsidian://open?vault=<id>&file=<rel-from-wiki-root>`）。caller（`codebus-cli/src/commands/lint.rs`）由 `vault_paths(repo).wiki` 提供。

text 格式調整：

| 部位 | use_emoji=false (current) | use_emoji=true (new) |
|---|---|---|
| clean header | `ok 5 pages + 2 nav files scanned, no issues\n` | `✅ 5 pages + 2 nav files scanned, no issues\n` |
| issue header | `# 5 pages + 2 nav files scanned, 1 error(s), 0 warning(s)\n` | `🔍 5 pages + 2 nav files scanned, 1 error(s), 0 warning(s)\n` |
| error path lead | `x ` | `✗ ` |
| warn path lead | `! ` | `⚠ ` |
| error issue line | `   error: <msg> [<rule>]\n` | `   \x1b[31merror:\x1b[0m <msg> [<rule>]\n`（紅色標籤） |
| warn issue line | `   warn:  <msg> [<rule>]\n` | `   \x1b[33mwarn: \x1b[0m <msg> [<rule>]\n`（黃色標籤） |

OSC 8 wrap 在 path lead 行：

```
ESC ] 8 ; ; obsidian://open?vault=<id>&file=concepts/foo.md ESC \ wiki/concepts/foo.md ESC ] 8 ; ; ESC \
```

URL 編碼：`vault_id` 與 `file` 路徑都做 percent-encoding（`%20` for space 等）。`vault_id` 為 `None` 或 `use_hyperlinks=false` 時 fallback 為純文字（無 OSC 8 escape）。

### Init progress 的 11 → 5 行轉換

| 既有 11 行 | 對應 banner |
|---|---|
| `✓ vault layout: <path>` | （吸收進 `Banner::Start { repo_path }`） |
| `✓ raw mirror: N files, B bytes, P PII matches` | `Banner::SyncDone { files, mib, elapsed_ms }` |
| `✓ vault internal .gitignore: ensured` | （細節，移到 debug；default 不印） |
| `✓ vault git: nested repo initialized` | （細節，移到 debug） |
| `✓ schema file: wrote .codebus/CLAUDE.md` | （細節，移到 debug） |
| `✓ manifest: wrote .codebus/manifest.yaml` | （細節，移到 debug） |
| `✓ skill bundles: 6 written, 0 already present ...` | （細節，移到 debug） |
| `✓ vault settings: wrote .codebus/.claude/settings.json` | （細節，移到 debug） |
| `✓ vault git: committed <sha7> "init: codebus vault"` | `Banner::CommitDone { sha7 }` |
| `✓ global config: wrote ~/.codebus/config.yaml` | （細節，移到 debug） |
| `✓ codebus init complete` | `Banner::Done { wiki_path }` + `Banner::Hint { wiki_path }`（如果 obsidian register 成功） |
| 內部 PII scan | `Banner::PiiSummary { scanner, scanned, hits, action }` |

default init 變 5 條 banner（Start / SyncDone / PiiSummary / CommitDone / Done [+ Hint]）；`--debug` 仍印全部 11 條 `✓ ...` 細節 + banner。

### debug mode 共存契約

`--debug` flag 不影響 banner 印（永遠印）；只額外打開：

1. 既有的 `[debug] xxx` lines（不變）
2. 既有「細節 progress lines」（init 那 11 條被 banner 取代後，在 debug mode 仍印）

實作：每個既有 `println!("✓ ...")` 改為 `if debug { println!("✓ ...") }` — 一行 if guard 包住即可。

## Implementation Contract

#### Behavior

##### Default 模式輸出形狀

- `codebus init` (default)：
  ```
  🚌 來囉來囉~ CodeBus 駛入 <repo>...
  ✓ 同步完成 (12 檔, 0.5 MiB, 230 ms)
  🛡 PII：regex_basic, scanned 12, hits 0, action mask
  📌 commit <sha7>
  🎉 掰掰~下車囉！wiki 已生成於 <wiki_path>
  💡 請用 Obsidian 開 <wiki_path>
  ```
  （6 行；`Hint` 只在 obsidian register 成功時印）
- `codebus goal "..."` (default)：
  ```
  🚌 駛入 <repo>...
  🎯 任務目標：<goal>
  ✓ 同步完成 ... (僅 needs_resync 時)
  [agent 自己的 stdout passthrough — 不在 codebus 控制]
  🔍 lint 中... (僅 fix_cfg.enabled 時)
  ✓ lint：0 errors, 0 warnings (45 ms)
  📌 commit <sha7>
  🎉 完成
  ```
- `codebus query "..."` (default)：精簡到 `🚌 駛入...` + agent passthrough + 結束（無 commit、無 lint）
- `codebus fix` (default)：`🔧 fix iter 1/1...` + agent passthrough + `✓ lint：` + `📌 commit`
- `codebus lint` (default)：既有 text 結構，加 emoji + color + OSC 8

##### Debug 模式輸出形狀

`--debug` 加在任一 verb：default 全 banner + 既有 `[debug] xxx` lines + 既有細節 `✓ ...` progress lines（init 11 行、其他 verb 各自的明細）。

##### lint text 範例（emoji + color + OSC 8 開）

```
🔍 5 pages + 2 nav files scanned, 1 error(s), 0 warning(s)

✗ <OSC8 wrap>wiki/concepts/foo.md</OSC8 wrap>
   <RED>error:</RED> frontmatter parse failed [frontmatter-parse]
```

#### Interface / data shape

```rust
// codebus-core/src/render/mod.rs
pub mod banner;
pub mod options;
pub mod lint_text;
pub use banner::{Banner, format_banner, print_banner};
pub use options::RenderOptions;
pub use lint_text::format_lint_text;

// codebus-core/src/render/options.rs
pub struct RenderOptions {
    pub use_emoji: bool,
    pub use_color: bool,
    pub use_hyperlinks: bool,
    pub vault_id: Option<String>,
}
impl RenderOptions {
    pub fn detect() -> Self;
    pub fn detect_with_vault_id(vault_id: Option<String>) -> Self;
    pub fn no_styling() -> Self; // emoji=false, color=false, hyperlinks=false, vault_id=None
}

// codebus-core/src/render/banner.rs
pub enum Banner<'a> { /* 10 variants — see Decisions */ }
pub fn format_banner(banner: Banner<'_>, opts: &RenderOptions) -> String;
pub fn print_banner(banner: Banner<'_>, opts: &RenderOptions);

// codebus-core/src/render/lint_text.rs
pub fn format_lint_text(result: &LintResult, opts: &RenderOptions, wiki_root: &Path) -> String;

// codebus-core/src/vault/obsidian_register.rs (modified — add public lookup)
pub fn lookup_vault_id(wiki_path: &Path) -> io::Result<Option<String>>;

// codebus-core/src/wiki/lint/output.rs (modified — keep existing format_text byte-equal,
// add format_text_with_opts that delegates into render::lint_text)
pub fn format_text(result: &LintResult) -> String; // unchanged
pub fn format_text_with_opts(result: &LintResult, opts: &RenderOptions, wiki_root: &Path) -> String;
```

#### Failure modes

- **Obsidian config not found / not registered** → `lookup_vault_id` 回 `Ok(None)` → `RenderOptions.vault_id = None` → OSC 8 hyperlink 不 emit，純文字 path（觀察行為：lint 路徑行不可 click 但仍可讀）
- **Terminal 不支援 OSC 8**（`supports-hyperlinks` 偵測 false）→ `use_hyperlinks=false` → 同上 fallback
- **`NO_COLOR=1`** → ANSI escape 不 emit；emoji 仍 emit（emoji 不算 color）
- **stdout 被 pipe / redirect**（非 TTY）→ `use_emoji=false, use_color=false, use_hyperlinks=false` — 純 ASCII 輸出，不污染管線
- **`obsidian.json` 存在但 parse fail** → `lookup_vault_id` 回 `Err(io::Error)`，caller swallow 為 `None`（與「未註冊」同處理）
- **vault_id 含 `&` / `=` 等 URL meta 字元** → 編碼前先 percent-encode（藉 `urlencoding` crate 或手寫小型 encoder）

#### Acceptance criteria

- 全部新檔案 (`render/{mod,banner,options,lint_text}.rs`) 有 `#[cfg(test)] mod tests` 覆蓋：
  - `format_banner` 各變體 byte-equal expected string（emoji on / off）
  - `RenderOptions::no_styling()` 三 flag 全 false
  - `format_lint_text` clean / single error / multi error 三 case，emoji on/off 各驗一次；OSC 8 wrap 在 `vault_id=Some("vid")` 時 escape 字元出現，`None` 時純文字
- `obsidian_register::lookup_vault_id` 有 unit test：obsidian.json 缺、vaults map 缺、path 不在 vaults、path 在 vaults 四個 case
- 5 個 verb command（init / goal / query / fix / lint）整合測試（`codebus-cli/tests/`）驗證 default + `--debug` 兩模式輸出包含/不含對應 banner 字串：
  - default：含 `🚌` `✓ 同步完成` `🎉` 等 banner 字面量；不含 `✓ vault layout:` 等 init 細節行
  - debug：含 banner + `[debug]` lines + 細節 `✓ vault layout:` 等
- 整合測試 set `NO_COLOR=1` 驗證 lint 輸出不含 `\x1b[`；非 TTY pipe 驗證不含 `🚌` emoji

#### Scope boundaries

**In scope**:

- `codebus-core/src/render/` 新模組（4 檔）
- `codebus-core/src/vault/obsidian_register.rs` 新增 `lookup_vault_id` 公開函式
- `codebus-core/src/wiki/lint/output.rs` 新增 `format_text_with_opts`（不破壞既有 `format_text`）
- 5 個 cli command 入口改為 emit banner（且 `--debug` 守住既有細節 lines）
- 環境偵測（`NO_COLOR`、TTY、`supports-hyperlinks`）

**Out of scope**:

- `Stdio::inherit()` 或任何 spawn 行為改動
- stream-json parsing
- RunLog / token usage（→ `v3-run-log` follow-up）
- `--emoji` / `--no-emoji` flag、`NO_EMOJI` env、`config.yaml emoji:`
- banner 內部 ANSI color
- lint JSON format

## Risks / Trade-offs

- **Windows console emoji 渲染** → 多數現代終端（Windows Terminal / PowerShell 7 / VSCode integrated）支援 emoji；舊 cmd.exe 可能顯示 `?`。Mitigation：TTY 偵測即用，不另加 OS-specific 判斷；user 可在不支援的終端用 `< file` redirect（非 TTY 自動關 emoji）解決
- **OSC 8 hyperlink 在不支援終端顯示亂碼** → `supports-hyperlinks` crate 已涵蓋主流終端的能力查詢；偵測 false 時 fallback 純文字
- **vault_id 與 Obsidian 實際 vault 不一致** → `lookup_vault_id` 從 `obsidian.json` 讀 ground truth，比 `compute_vault_id` 算的更可靠；user `--no-obsidian-register` 過 init 後 obsidian.json 沒有對應 vault → `lookup_vault_id` 回 None → 退化為純文字（一致 fallback）
- **替換 11 行 `✓ ...` progress 是 BREAKING** → 任何 grep stdout 對特定 progress 字串的下游 tooling 會壞。Mitigation：proposal 標 BREAKING；`--debug` 仍印細節行（grepper 可改用 debug mode）

## Migration Plan

無 schema migration（純 stdout 輸出形狀變化）。BREAKING 範疇：

1. **init default 從 11 行 → 5 行 banner**：grep `✓ vault layout:` / `✓ skill bundles:` 等 stdout 字串的腳本會撈不到。文件化於 proposal What Changes 段；user 改跑 `--debug` 仍可見細節
2. **lint text 默 emoji + color**：JSON 模式不變；text 模式有 emoji（非 TTY 時自動關）

無回滾步驟 — 純 UI 改動，舊 binary 跑新 codebase 仍可正常編譯（API 都向後相容：`format_text` 簽章不變，新增 `format_text_with_opts` 並列）。

## Open Questions

無；discuss 階段已收斂。
