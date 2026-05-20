# Backlog: Wiki 頁面加按鈕直接開 Obsidian

**Date:** 2026-05-20
**Surfaced during:** roadmap review 2026-05-20（取代 wiki-graph-view 當下需求）
**Severity:** UX 補強（小但有感）
**Owner:** harry
**Status:** open

---

## 觀察

`codebus init` 已經把 vault register 進 Obsidian（`v3-roadmap.md:46`「Obsidian register」），但 app 的 Wiki preview / Workspace 內**沒有直接跳轉 Obsidian 的入口**。user 想看 graph / backlink / 進階編輯時要：

1. 自己開 Obsidian
2. 找到對應 vault
3. 再點到對應頁面

三步操作 friction 太高，等於用了 codebus 之後 Obsidian 就放著生灰。

跟原 [wiki-graph-view](2026-05-20-wiki-graph-view-backlog.md) backlog 的關係：那條想用 sigma.js + graphology 在 app 內做 graph view（中等工程量），2026-05-20 重新評估後決定**直接跳出去用 Obsidian** —— 既然 init 已經 register，按鈕兩步到位就好。原 graph-view backlog 已結案，留檔保決策脈絡。

## Proposed fix

Wiki preview 標題列旁邊加一顆按鈕：

```
┌─────────────────────────────────────────────────────────┐
│ wiki/modules/uv-lib.md           [✏️ Open in Obsidian] │
├─────────────────────────────────────────────────────────┤
│ # uv-lib                                                 │
│ ...                                                      │
```

點擊 → Tauri 走 `shell::open` 觸發 URL scheme：

```
obsidian://open?vault=<vault-name>&file=<relative-path>
```

- `<vault-name>` = `<repo>` 目錄名（init 時 register 用的名字）
- `<relative-path>` = wiki page 相對於 vault root 的路徑（URL encode）

### 沒裝 Obsidian 的 fallback

URL scheme 在沒裝的系統上會 silent fail。兩個選擇：

- **A**（推薦）：按鈕一律顯示，按下後 OS 自己處理（沒裝 → 跳「找不到應用程式」對話框 / 沒反應），不偵測
- **B**：偵測 Obsidian 安裝（macOS `/Applications/Obsidian.app` / Windows `%LOCALAPPDATA%\Obsidian` / Linux `which obsidian`），沒裝就 hide 按鈕

A 工程量輕、行為可預測；B 牽涉 cross-platform 偵測 + edge case。建議先 A，等真有 user 抱怨「按了沒反應」再加 B。

### 跨平台 URL scheme 確認

Obsidian URL scheme 在 macOS / Windows / Linux 都支援（Obsidian 官方協定）。Tauri `shell::open` 在三平台皆 ok（macOS `open`、Windows `start`、Linux `xdg-open`）。

## Tasks（粗估）

1. Wiki preview header 加按鈕 component（i18n + emoji icon）
2. Tauri IPC command：`open_in_obsidian(vault_name, relative_path)` → `shell::open`
3. URL encoding 處理（路徑含空白 / 中文 / 反斜線正規化）
4. vitest：按鈕 click handler / URL 拼接邏輯
5. 手動驗收：Windows 上點按鈕能正確開到對應 page

工程量：輕（半天）。

## Out of scope

- 自動偵測 Obsidian 安裝狀態（先採方案 A，hide-on-missing 是後續加碼）
- 從 Obsidian 反向操作回 codebus（沒這需求）
- 在 app 內自製 graph view（已結案，見 wiki-graph-view backlog）
- 改寫 Obsidian register 邏輯（init 時的 register 已 work，這條只動 GUI 跳轉）

## 何時動

跟 `v3-app-polish-ship` 同期或稍後皆可。獨立工程量極小、無依賴，可隨時插進去。
