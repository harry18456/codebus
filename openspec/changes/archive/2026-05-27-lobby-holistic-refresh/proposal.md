## Summary

把 04 Lobby 兩個畫面（04b empty / 04a vault list）的視覺與互動 gap 一次清完——layout 不再垂直置中、Quickstart 補 amber pill 與 mono 編號、section label 跟 vault 字面換成新 token 與 wording、加 04b hero 🚌 idle micro-motion。

## Motivation

Phase 2 design-foundation tokens（typography +1 級 / border 對比 / `SectionLabel` 元件）已落地，Phase 3A i18n 收尾後 Lobby 字串都進 bundle，現在是把累積的視覺 gap 一次收掉的最佳時機。Lobby 是新 user 第一眼看到的畫面、ROI 最高；AUDIT 已把 4 區塊（empty polish / section label / vault 詞拿掉 / hero idle motion）的決策鎖好，sequencing 在 `codebus-app/design-handoff/AUDIT.md` 的 "Phase 4A · lobby-holistic-refresh" 段。

## Proposed Solution

範圍切 4 區塊，全部集中在 `codebus-app/src/components/lobby/*` + 少量 i18n bundle / globals.css：

- **A · 04b empty polish**：Lobby 主 container 從 `items-center justify-center` 改 `flex flex-col` + Quickstart 內容自然上排（G1）；Quickstart step2 把 example wording 包成 amber-tinted mono pill（G2，`bg-accent-tint` + `text-accent` + 細 padding）；步驟編號 `1.` `2.` `3.` 改 mono 數字無句點配 `text-fg-tertiary`（G3）；Quickstart card 密度跟 G3 一起收（G7）。G6 footer 樣式本體（border-top + bg-sunken）在 BottomStrip 已 land，本 change 不再動。
- **B · G4 中文 section label**：04a「近期 VAULT」與 04b「快速開始」改用既存 `SectionLabel` 元件並套 `variant="caps"` 或 default（caps 變體在 Phase 2 已 build），取代現況直接寫 `uppercase tracking-[0.12em]` 對中文無效的問題。
- **C · Vault 詞從 UI 拿掉（G-copy-2 + G-04a-1）**：保留現有 3 個 i18n key（`lobby.topbar.newVaultButton` / `lobby.populated.sectionLabel` / `lobby.populated.dragTip`），只改 zh 與 en value——topbar `+ 新增` / `+ Add`；section `最近` / `Recent`；drag tip 對應新 wording 不含 vault 字面。VaultCard 補可見 kebab（hover 顯示）、保留 right-click context menu 當 shortcut。
- **D · ODI-1 Bumpy road**：04b hero 56px 🚌 加 idle micro-motion——垂直 2px bob + 水平 1px jitter，1.4s loop，無 rotation，純 CSS `@keyframes` 加進 globals.css；`@media (prefers-reduced-motion: reduce)` fallback 完全靜態。

驗收一律走真實 CDP smoke（zh + en locale 各一遍）+ `pnpm tsc` + `pnpm test`，截圖存 `codebus-app/scripts/.lobby-refresh-smoke/`。詳細 site 校準與 task list 進 design.md / tasks.md。

## Non-Goals

- 不動 Workspace（Phase 4B 範圍）、Goals tab、Quiz tab（Phase 4C 範圍）
- 不做 v1.1 spec 才會落地的 Lobby 改動（LoadingOverlay 變動、ChatWidget 等，留給 Phase 6）
- CLI / `VaultEntry` data model / config YAML 保留 `vault` 詞——本 change 只動 UI 顯示層
- 不再改 G-copy-1 副標與 Quickstart 步驟 wording（已在 Phase 3A 之前決定，wording 是否真正 landed 由其它 change 收尾）
- ODI-1 不做大幅 transform / opacity 動畫；不加路面虛線、輪子、進場過場、其他畫面 🚌 動畫
- 不重新命名既存 i18n key（per AUDIT G-copy-2 line 477「key 命名不動、只改 value」）

## Alternatives Considered

- **ODI-2 fullscreen ambient background**（極淡 dot grid）：留作備案，等 G1 修完真機 1920×1080 仍空才考慮，不在本 change
- **新增 `lobby.topbar.addAction` / `lobby.section.recent` / `lobby.dragTip.text` 三組新 key**：trailer 原本這樣寫，但 AUDIT 明確要求 key 不動只改 value，避免 rename 牽動測試與 grep consumer

## Impact

- Affected specs: `app-shell`（Lobby empty state / populated list / quickstart / drag tip 等 requirements 微調 wording 與 layout 約束）
- Affected code:
  - Modified:
    - codebus-app/src/components/lobby/Lobby.tsx
    - codebus-app/src/components/lobby/EmptyState.tsx
    - codebus-app/src/components/lobby/VaultCard.tsx
    - codebus-app/src/i18n/messages.ts
    - codebus-app/src/styles/globals.css
  - New:
    - codebus-app/scripts/.lobby-refresh-smoke/ (CDP smoke 截圖目錄)
  - Removed: (none)
