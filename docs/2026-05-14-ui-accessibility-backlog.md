# Backlog: UI 無障礙（對比度 + 鍵盤導航）

**Date:** 2026-05-14
**Surfaced during:** backlog 討論（v3-app-chat-cmdk apply 期間）
**Severity:** accessibility gap
**Owner:** harry
**Status:** parked

---

## 觀察

codebus-app 目前的 UI 在兩個無障礙軸上有系統性缺口：

### 對比度

整個 app hard-coded dark theme，部分文字 / icon 對比度偏低：

- sidebar 淡色 metadata text（vault path、上次開啟時間）使用灰階接近背景
- activity stream tool_use 行、thought fold 摘要文字視覺層級過淡
- disabled 狀態按鈕對比度尤其低

WCAG AA 要求一般文字 ≥ 4.5:1、large text ≥ 3:1。目前未系統性驗證任何元件。

### 鍵盤導航

- focus ring 部分元件缺失或被 `outline-none` 蓋掉
- modal（Settings）/ drawer（Chat Widget）的 focus trap 未實作
- sidebar tab 切換、vault card 選取未支援純鍵盤操作
- Escape 關 modal 行為不一致

## Proposed fix

新提一條 change：`v3-app-accessibility`

### 對比度

1. 用 `axe-core` / `Colour Contrast Analyser` 系統掃全 UI 元件
2. 調整灰階 token，確保所有文字通過 WCAG AA
3. 重點元件：sidebar metadata、stream dim text、disabled states

### 鍵盤導航

1. 全元件補 visible focus ring（Tailwind `focus-visible:ring-2`）
2. modal / drawer 實作 focus trap（用 `@radix-ui/react-focus-trap` 或 shadcn 內建）
3. sidebar tab 加 `role="tab"` + 方向鍵導航
4. 統一 Escape 關閉行為（modal、drawer、dropdown 都 respond）

### Tasks（粗估）

1. axe-core audit script：自動跑 + 輸出違規清單
2. 灰階 token 調整（Tailwind config）
3. focus ring 補完（全元件掃）
4. modal / drawer focus trap
5. sidebar 鍵盤導航
6. Escape 統一處理
7. E2E accessibility 測試（playwright axe-core integration）

工程量：中（2-3 個半天；視 audit 結果可能更多）。

## Out of scope

- Screen reader 完整支援（ARIA 全面補完）— 超出 v1 範圍
- Light theme 對比度驗證 — v1 hard-coded dark
- 字體縮放（獨立 backlog：`app-font-scale`）

## 何時動

優先序低於 E / F，建議在 F `v3-app-polish-ship` 內或緊接在後。
accessibility audit + fix 天然跟 visual QA 一起跑效率最高。

## 替代：什麼都不做

v1 主要用戶是 dev / power user，鍵盤操作需求相對低。
若 ship 後無實際反映可維持 parked。
