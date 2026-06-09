# Backlog: 全域 font-scale / accessibility text size

**Date:** 2026-05-14
**Surfaced during:** v3-app-chat-cmdk discuss（layout 規格設計時 user 提到「字都很小之後要調大也考慮一下」）
**Severity:** accessibility gap
**Owner:** harry
**Status:** parked

---

## 觀察

整個 codebus-app 普遍使用偏小字級 — 比 Tailwind 預設 `text-sm (14px)` 還小一階：

| 位置 | 字級 | 證據 |
|---|---|---|
| Workspace sidebar (back btn, vault path) | `text-[11px]` / `text-[12px]` | `Workspace.tsx:142, 158, 241` |
| Vault card 上次開啟時間 / missing badge | `text-[11px]` / `text-[10px]` | `VaultCard.tsx:51, 56, 63` |
| Settings modal field label | `text-[11px]` | `SettingsModal.tsx:154`（grid `text-xs` ≈ 12px） |
| Run detail metadata | `text-[11px]` | `RunDetailRunning.tsx:85` |
| Activity stream tool_use 一行 | `text-[12px]` | `ActivityStreamItem.tsx:40, 49` |
| Endpoint section verb row | `text-[11px]` | `EndpointSection.tsx:435, 446` |

對「想看更大字」的使用者（accessibility / 老花 / 高 DPI 螢幕）目前沒有調整管道。

## 為什麼需要

1. **Accessibility** — WCAG 建議 body text 至少 16px、可調至 200%。目前 11-12px 在 default zoom 下對視力不佳者吃力。
2. **Per-user 偏好** — 不同用戶喜好不同密度（dev 喜歡 dense 看更多 / 一般用戶喜歡 spacious）。
3. **Layout 互動** — chat widget (v3-app-chat-cmdk) drawer 寬度若 hard-coded px，font scale 調大後內容會 wrap 變醜。要 rem-based + 全域 scale 變數才能 layout 等比調整。
4. **High-DPI 螢幕** — Windows 150% scaling 已是預設、Mac retina 2x 也是；目前 px-based 設計在不同 scaling factor 上不一定符合 design intent。

## Proposed fix

新提一條 change：`v3-app-font-scale`

### Spec / Design

- 全域 CSS custom property `--app-font-scale`，可選值 `1.0` / `1.15` / `1.3` / `1.5`（4 檔）
- Root html `font-size: calc(16px * var(--app-font-scale))`
- 所有 hard-coded `text-[Xpx]` 改成 rem-based（例：`text-[11px]` → `text-[0.6875rem]` 或定義 token `text-xxs`）
- Tailwind config 新增 `text-xxs (0.6875rem)` / `text-2xs (0.75rem)` token，取代 arbitrary `text-[11px]` / `text-[12px]`
- 寬度單位：所有 `w-[Xpx]` / `min-w` / `max-w` 改 `rem`（影響 sidebar 200px、settings grid 168px、chat widget 22rem 等）
- Settings modal 新增「Text size」field（4 個 radio: `Compact` / `Default` / `Large` / `Extra large` 對應 4 個 scale）

### Migration

- 既存所有 `text-[\dpx]` 一次掃過、機械式轉成 rem token
- Wiki preview 用 Milkdown 自己 typography（已 rem-based），不動
- 沒有 user-facing migration warning（純內部重構）

### Tasks（粗估）

1. spec ADDED `app-font-scale`：定義 4 個 scale 檔位 + Settings UI 規格
2. Tailwind config token 新增（`text-xxs` / `text-2xs` / 或 `text-1` `text-2` `text-3` 三檔語意化）
3. Codemod：grep 全 codebase `text-\[\d+px\]` / `w-\[\d+px\]` 轉 rem
4. Settings modal 加 `Text size` field（serde 存進 `~/.codebus/config.yaml` `app.text_scale: 1.0`）
5. App root component 讀 config 設 `--app-font-scale` CSS var
6. E2E 截圖比對 4 個 scale 各自 layout 正確（截 Lobby / Workspace Goals / Wiki / Settings 四個關鍵頁）

工程量：中（2-3 個半天 + 視覺 QA 工時不可忽略）。

## Out of scope

- 不改 Wiki preview（Milkdown）內部字級 — 已有自己的 typography token
- 不做 dark/light theme 切換（v1 hard-coded dark）— 是獨立軸
- 不做 RTL / 多語系 layout 調整
- 不做 per-vault text size override（global only）

## 何時動

優先序低於 D `v3-app-chat-cmdk` 與 E `v3-app-quiz`。

時機點選：
- **跟 D 一起**：chat widget rem-based 是 D scope；font-scale infrastructure 順手做（風險：D 變胖）
- **D 之後、E 之前**：獨立 change ship，影響面廣（全 UI repaint）但邏輯隔離
- **F polish-ship 內**：跟其他 polish 綁，最後一次掃完（推薦）

最後一個選項最務實 — font-scale 屬於 polish，F 階段一次處理跨平台驗證 + visual QA + font-scale 三件事比較有規模效應。

## 替代：什麼都不做

對 dev / power user（多數 v1 user）11-12px 是 dense info 的 sweet spot，不一定要做。
若 v1 ship 後沒實際 user 反映字太小，可永久 parked。
