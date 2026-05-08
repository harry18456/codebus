## Context

`codebus` Phase 1 design 第 861 行明確把「OSC 8 hyperlink for `[[wikilink]]`」標為 Phase 2 工作。Legacy TypeScript 版（`legacy/ts-src/src/ui/render.ts:114`）已實作 `chalk.cyan.underline` wikilink 染色，但 Rust port 的 `codebus-core/src/render/renderers/terminal.rs:5-7` 至今寫著 "Color formatting is intentionally deferred"——連底色染色都還沒做，更談不上 hyperlink。Karpathy 5-folder taxonomy 已穩定（`concepts/`, `entities/`, `modules/`, `processes/`, `synthesis/` + 3 special + `goals/`），slug index 有清楚的目標來源。

進 propose 前已做完一輪 Windows 11 + Windows Terminal + Obsidian 1.x 的實機 spike，驗證了關鍵假設（見下面 Decisions）；macOS / Linux 的 Obsidian config 行為**僅做紙上設計**，留 follow-up 驗證。

## Goals / Non-Goals

**Goals:**

- CLI 印的 thought 文字含 `[[wikilink]]` 時，現代終端使用者 Ctrl+Click 可直接跳到 Obsidian 對應頁
- 補上 legacy TS 已有但 Rust port 沒做的輕 markdown 染色（bold / inline code / wikilink）
- `codebus --repo X` 第一次 init 自動把 `.codebus/wiki/` 註冊到 Obsidian，使用者不用手動加 vault
- 不支援的終端 / 沒裝 Obsidian / Obsidian 在跑時都優雅退化（不爆、不變垃圾字元、不破壞既有設定）
- 跨平台路徑設計（Win / macOS / Linux）

**Non-Goals:**

- 不做 `file://` URL fallback（系統 .md handler 不可預期，故事斷）
- 不解 Obsidian sidebar 顯示「raw/」雜訊（vault path 改指 `.codebus/wiki/` 後不存在這問題）
- 不解 vault name 撞名問題（OSC 8 URL 用 `vault=<sha256-id>` 變體，根本不靠 name）
- 不嘗試 Obsidian 跑著時 race-safe 寫入（複雜度高、收益低，採偵測 + skip + hint）
- 不做完整 markdown rendering（headings、bullet、code block 等），只做 bold / inline code / wikilink 三種高頻 marker
- 不做 Obsidian community plugin（Tauri tutorial app 之前不會走 plugin 路線）

## Decisions

### URL scheme：vault id 變體當主路徑，path 變體當文件記載 fallback

**選**：`obsidian://open?vault=<sha256-id>&file=<type>/<slug>`

**為什麼**：
- URL 短、跨 OS 一致（不帶絕對路徑）
- vault id 由 codebus 自己算（path SHA256 前 16 hex）+ 自己寫進 obsidian.json，整條鏈 self-contained
- 即使使用者在 Obsidian 內改 vault display name，id 不變，連結仍能跳

**alternatives 拒因**：
- `obsidian://open?path=<URL-encoded abs>`：URL 長、跨 OS 路徑分歧，且實測 spike 中兩個變體**都成功**（差異只是優劣不是能不能用）。此變體在 spec 中以「文件記載的 fallback 策略」存在，不主動實作；若未來 Obsidian 改 `vault=` 解析行為（官方文件說 vault name，認 id 是 undocumented 行為，見 Risks），可切換
- `obsidian://search?vault=&query=<slug>`：點下去跳搜尋頁，多一步、體驗差

### Vault path 指 `.codebus/wiki/`，不是 `.codebus/`

**為什麼**：使用者視角「想看 wiki」，`.codebus/raw/` / `logs/` / `goals.jsonl` / `CLAUDE.md` / nested `.git/` / `.obsidian/` 都不該在 Obsidian 顯示。指 `.codebus/wiki/` 後 sidebar 內容就是 5 folder + 3 special + `goals/`，乾淨。

**alternatives 拒因**：
- 指 `.codebus/`：要靠 `userIgnoreFilters` 排除子資料夾才能藏雜訊，但官方 setting 只影響搜尋 / Quick Switcher / Graph，sidebar 仍顯示——治標不治本
- 指 repo root：raw source code 全進 vault，Obsidian 把所有 `.md` 當 wiki 解析，graph 更糟

副作用：多個 codebus repo 註冊後 vault name 都是 `wiki`（path 末段），但 URL 用 vault id 變體不靠 name，問題不需要解。

### Vault id 算法：`SHA256(abs_path.to_lowercase())[:16]`

**為什麼**：
- 穩定（重複跑 init 不產生重複 entry）
- 不衝突（不同 path → 不同 hash）
- 大小寫不敏感（Win 路徑 `D:\` 跟 `d:\` 視為同 vault）
- 16 hex chars 跟 Obsidian 自己生的 random id 長度一致，URI 解析行為觀感對齊

**alternatives 拒因**：
- 用 Obsidian style 的 random id：每次 init 都產不同 id，重複註冊
- 用完整 SHA256：32 hex chars，URL 變長，無收益

### Idempotent 寫入：reuse same-path entry 既有 id

`obsidian.json` 寫入前掃 `vaults`，發現有 entry path 等於目標 path（大小寫不敏感正規化後）→ reuse 既有 entry id（不論是 codebus 還是使用者手動加的），只更新 `ts`；vault 完全不存在 → 寫新 entry 用 `SHA256[:16]` id。

**為什麼**：使用者可能在 Obsidian 介面手動加過 `.codebus/wiki/` 為 vault（產 random id 而非 SHA256 id），codebus 再寫一份 SHA256 id entry 會在 obsidian.json 出現兩個指向同 path 的 entry，且 OSC 8 URL 的 `vault=<sha256-id>` 不一定能對到使用者實際開的那個。Reuse 既有 id 解決這問題；OSC 8 URL 改用「掃完 vaults 後拿到的 effective id」（不一定是 SHA256，可能是使用者既有 random id）印出。

### Obsidian 在跑時：偵測 + skip + hint，不硬寫

寫 `obsidian.json` 前先 `Get-Process Obsidian` (Win) / `pgrep -x Obsidian` (macOS/Linux)，偵測到就跳過寫入並印一行 hint：

```
💡 Obsidian 正在執行，跳過自動註冊。請關閉 Obsidian 後重跑 `codebus --repo X` 或在 Obsidian 內手動加入 .codebus/wiki/ 為 vault。
```

**為什麼**：spike 觀察到 Obsidian 在跑時，使用者操作 vault（切換、開關）會把記憶體 vault list 整份回寫。如果 codebus 寫入時機剛好夾在它讀進記憶體之後又回寫之前，我們的 entry 會被丟失（race）。實測在「使用者不主動操作 vault」的情況下不會 race，但無法保證；偵測 + skip + hint 是最務實。

**alternatives 拒因**：
- 硬寫不偵測：使用者不知道為什麼點不開，silent fail，體驗差
- 用檔案鎖：Obsidian 不釋放鎖，無法協調
- 重啟 Obsidian process：副作用大、會干擾使用者其他 vault 工作

### 終端能力偵測：`supports-hyperlinks` crate

`supports-hyperlinks = "3"`（Cargo crate）依環境變數 + tty 偵測終端是否支援 OSC 8（涵蓋 Windows Terminal、iTerm2、VSCode integrated terminal、GNOME Terminal、Kitty、WezTerm 等主流）。

**為什麼**：
- 跨 OS 偵測（不用自己 sniff `WT_SESSION` / `TERM_PROGRAM` / `KITTY_WINDOW_ID` 之類）
- 維護成本外包給上游
- 不支援的終端只染色不包 OSC 8 escape（使用者看到 `[[buddy-cli-commands]]` 藍底線但不能點，不會看到 `]8;;obsidian://...` 垃圾字元）

**alternatives 拒因**：
- 自己 sniff：跨 OS 適配條件每加一種終端就要更新，技術負債
- 一律印 OSC 8：少數老終端會把 escape 當可見字元印出垃圾

### Slug index 時機：run 啟動時 build once

每次 `codebus --repo X --goal/--query` 進入時，掃 `.codebus/wiki/{concepts,entities,modules,processes,synthesis,goals}/*.md` + `wiki/{overview,index,log}.md` 建 `HashMap<slug, (PageType, rel-path)>`，注入 `RenderOptions::slug_index`。

**為什麼**：
- 啟動掃 wiki 是 O(N) 一次成本（典型 vault N < 200 頁），無感
- Render layer 拿到 const reference 解析 wikilink 到 type folder
- Agent 同 run 內新生的 wiki 頁這次點不到（index 在 run 啟動時拍快照），下次 run 才有；可接受 trade-off

**alternatives 拒因**：
- 每次 render `[[slug]]` 即時 lookup vault：I/O 太頻繁
- 不做 index，wikilink 退化到 `obsidian://search?query=<slug>`：點下去跳搜尋多一步，體驗降級

### 跨 OS Obsidian config 路徑

| OS | obsidian.json 位置 |
|---|---|
| Windows | `%APPDATA%\obsidian\obsidian.json`（即 `~/AppData/Roaming/obsidian/`）|
| macOS | `~/Library/Application Support/obsidian/obsidian.json` |
| Linux | `~/.config/obsidian/obsidian.json` |

封裝在 `codebus-core/src/obsidian/config_path.rs::resolve()` 用 `dirs` crate（codebus 已用）查 `config_dir()` + 加 `obsidian/` 子路徑。

**Note**：Linux Flatpak 安裝的 Obsidian config 在 `~/.var/app/md.obsidian.Obsidian/config/obsidian/`，**不在這次涵蓋範圍**——若使用者用 Flatpak 安裝，auto-register 會 silent fail（寫到非 Flatpak path 但 Flatpak Obsidian 看不到）。Open Question 之一。

### `--no-obsidian-register` opt-out flag

`codebus --repo X --no-obsidian-register` 跳過自動註冊。

**為什麼**：使用者可能：(a) 不用 Obsidian、(b) 已有複雜 vault 設定不想被動、(c) CI / docker 環境沒 Obsidian。

flag 預設 false（即「預設自動註冊」），降低首次使用門檻。

## Risks / Trade-offs

[**Risk: Obsidian URI `vault=<id>` 認 id 是 undocumented 行為**] → Mitigation: spec-level 以 `vault=<id>&file=<rel>` 為主，但保留 `path=<abs>` 變體當「若 Obsidian 改行為時的逃生口」。實測 Obsidian 1.x 行為穩定，可能是 fallback `name → id` 順序。在 design 內記為 risk，遇到變化時 codebus-core/src/render/markdown_style.rs 改一行 URL 模板即可切換

[**Risk: macOS / Linux 沒實機驗證**] → Mitigation: phase 1 主要驗證在 Windows，macOS/Linux 走 follow-up E2E。`config_path::resolve()` 有 `cfg(target_os = ...)` 分支單元測試，但 Obsidian 啟動行為僅紙上設計。Linux Flatpak 路徑差異記為 known limitation

[**Risk: 使用者用 Obsidian 介面手動加同 vault，產生重複 entry**] → Mitigation: idempotent 寫入策略已處理（reuse same-path entry id），但若使用者**先**在 Obsidian 加（random id）、**再**跑 codebus init，OSC 8 URL 會用使用者既有 random id 印出（codebus 掃 vaults 後拿 effective id）。codebus 不主動覆蓋使用者既有 entry

[**Risk: Obsidian 跑著時 race overwrite codebus entry**] → Mitigation: 偵測 + skip + hint。spike 觀察使用者不主動操作 vault 時 Obsidian 不會回寫，但仍以「偵測到就跳過」為安全網

[**Risk: 不支援 OSC 8 的終端 + `use_color=true`**] → Mitigation: `supports-hyperlinks` crate 偵測，hyperlinks 與染色為兩個獨立 flag（`hyperlinks` ⊆ `use_color`）。不支援終端只染色不包 OSC 8 escape

[**Risk: vault path 末段為 `wiki`，多 repo 註冊後 picker 列出多個「wiki」**] → 接受。OSC 8 URL 用 vault id 變體不靠 name，picker 視覺重複只影響使用者手動切 vault 的選擇——可在 Obsidian 介面改 display name 解決，不在 codebus 範圍

## Migration Plan

- 既有 codebus vault 不受影響（init 是 missing-才寫；首次跑後續 init 觸發 `obsidian register` step）
- 首次 init 流程在 PII filter / lint 之前插入 obsidian register step（這個 step 失敗 != init 失敗，只 warn）
- 既有使用者跑新版的第一個 `codebus --repo X` 觸發 register；`--no-obsidian-register` 可跳過
- 沒裝 Obsidian 的使用者：偵測 obsidian.json 父目錄不存在 → silent skip（不印錯）
- Rollback：`obsidian.json` 寫入前備份到 `obsidian.json.codebus-bak`（最後一次成功寫入前的版本），手動還原可用

## Open Questions

- macOS / Linux 上 OSC 8 + Obsidian URI 的實機行為（特別是 Linux 不同 DE / Wayland vs X11、Flatpak Obsidian）——是否在這次 change 內補實機 E2E，還是留 follow-up？
- 是否要支援 Linux Flatpak Obsidian path？（`~/.var/app/md.obsidian.Obsidian/config/obsidian/`）目前傾向 known limitation 不做，但若使用者報告才補
- 是否要把 `obsidian register` 抽成獨立 subcommand（`codebus register-obsidian --repo X`）讓沒跑過 init 的既有 vault 也能補註冊？目前傾向放在既有 init flow，不另開 subcommand
