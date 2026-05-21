# Backlog: CLI `[[slug]]` 可點連結 + 可設定連結目標（app / obsidian）

**Date:** 2026-05-21
**Surfaced during:** discuss 2026-05-21（「cli 連結導向 obsidian vs app」——查證後確認 v3 已弄丟 v2 的 `[[slug]]`→obsidian 連結能力）
**Severity:** capability regression（v2 有、v3 弄丟）+ capability enhancement（連回 app）
**Owner:** harry
**Status:** open

---

## 觀察

v2 的 CLI agent 串流會把 thought 文字裡每個 resolvable `[[slug]]` 用 OSC 8 包成 `obsidian://open?vault=<id>&file=<rel>` 超連結（legacy `v2-rust/codebus-core/src/render/renderers/terminal.rs` 註解明載，不支援的終端降級為純樣式）。**v3 把這能力弄丟了**——`stream_event.rs` 是從 v2 `format_event` 搬過來的，但沒搬 `[[slug]]` 的 OSC 8 包裝。

v3 現況（discuss 實查）：

| Surface | 現在連到哪 |
|---|---|
| CLI `codebus lint` | OSC 8 → `obsidian://`，但包的是**檔案路徑**（`wiki/foo.md`），不是 `[[slug]]`（`lint_text.rs:17-20`） |
| CLI `goal`/`query` 串流 | **無連結**——`stream_event.rs` 整支不碰 hyperlink |
| CLI `chat` | **無連結**——raw `println!`（見 [chat-display-polish](2026-05-21-chat-display-polish-backlog.md)） |
| App wiki 分頁 | `[Open in Obsidian]` 按鈕 → `obsidian://`（`ipc/wiki.rs:203`） |
| App chat | wiki link 走**站內**導航（`ChatTranscript.tsx:399`），不是 obsidian |

所以使用者記得的「CLI 出現 `[[xxxx]]` 點了開 obsidian」是 **v2 行為**，v3 目前做不到。

## Decision（discuss 結論）

補回「CLI 把 `[[slug]]` 渲染成可點連結」，**並讓連結目標可由 config 控制**：

- **新增 config CLI 區段**：`~/.codebus/config.yaml` 加一個 `cli` 區段（新 sub-module `codebus-core/src/config/cli.rs`，比照既有 `lint_fix`/`pii`/`quiz` 的 sub-module 模式：自帶 `Default` + loader + forward-compat 容錯）。變數 `link_target`：**預設 `app`**，可改 `obsidian`。
- **app 連結目標 = 新 `codebus://` deep-link 協定**（見下方 hard dependency）。
- **obsidian 連結目標 = 沿用現成路徑**（`obsidian_register::lookup_vault_id` + OSC 8）。
- **app chat 不在本條範圍**：`[[slug]]` 維持站內導航（沿用既有 `onWikiLinkClick`）。連結目標 config 只管 **CLI** surface。

### Hard dependency：`codebus://` deep-link 協定（必須先做）

「連回 app」需要 app 註冊自訂 URL 協定——app 目前**沒有**任何自訂協定（`ipc/wiki.rs` 只會呼叫別人的 `obsidian://`）。因為 `link_target` 預設是 `app`，**協定沒做好之前預設值是壞的（點了沒反應）**。discuss 已決定：**先建協定，再讓 app 當預設**。

協定工作含：
- 註冊 `codebus://` scheme（跨 Windows / macOS / Linux，Tauri deep-link plugin 或各平台原生註冊）
- deep-link 喚醒：app 未開→啟動、已開→聚焦既有視窗（single-instance）
- 路由：`codebus://wiki/<slug>` → 切 wiki 分頁 + `loadPage`（沿用既有 `ipc/wiki.rs` 的 slug→page 解析）

### 需在 propose 階段留意的張力

`render/options.rs:17-21` 白紙黑字記著「v3 **刻意**拿掉 v2 的渲染類 config 旋鈕（away from v2's 5-level priority chain）」。本決定對它開例外——理由：**`link_target` 是路由語意（連結指向哪個 app），不是 styling 偏好（emoji/color）**，且有明確使用需求。propose 時要把這個區分講清楚，別把它跟被否決的 styling 旋鈕混為一談，也別重蹈 v2 五層優先鏈。

## Proposed tasks（粗估）

1. **deep-link 協定（先）**：註冊 `codebus://` + 喚醒/聚焦 + `codebus://wiki/<slug>` 路由到 wiki 分頁。跨 OS 驗證。**工程量：重。**
2. **config CLI 區段**：`config/cli.rs` + `link_target: app | obsidian`（預設 `app`）+ forward-compat 容錯 + 載入接線。**工程量：輕。**
3. **CLI linkify（補回 v2）**：`stream_event.rs`（thought）+ `chat.rs` 把 resolvable `[[slug]]` 用 OSC 8 包成連結；URL 依 `link_target` 組 `codebus://wiki/<slug>` 或 `obsidian://open?...`。需要 slug→（resolvable? / path）解析——v3 無 SlugIndex（只 legacy 有），決定要建索引還是走「一律 linkify、由目標 app 自己處理不存在」的簡化版。**工程量：中。**
4. 降級：`use_hyperlinks` false / 目標不可用時退純文字（比照 lint 既有降級）。
5. 測試 + 跨 OS 手動驗收（協定喚醒、兩種 target、降級）。

工程量總計：**重**（協定那塊吃掉大半）。

## Out of scope

- App chat / wiki 分頁的 `[[slug]]` 行為（維持站內導航；chat 的渲染缺口見 [chat-display-polish](2026-05-21-chat-display-polish-backlog.md)）。
- GFM 表格渲染（在 chat-display-polish）。
- 把 `link_target` 擴成 styling 類 config（明確避開 `options.rs` 的反向決策）。

## 何時動 / 優先序

協定那塊是重工程且 v3 render 哲學是「無 config」，動工前用 `/spectra-propose` 正式走一遍，把「為何對 no-render-config 原則開例外」「協定 vs linkify 的拆分與先後」談定。`[[slug]]` linkify 本身（task 3）若先想看到效果，可先以 obsidian 為唯一目標做（task 1/2 之前），但那就回到 v2 行為、不含 app 閉環——是否值得先做半套由 propose 決定。
