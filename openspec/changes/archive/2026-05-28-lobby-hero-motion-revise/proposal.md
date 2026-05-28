## Why

2026-05-28 v1.1 implementation acceptance pass 中、user 親自試了原 spec lock 的「Idling in place」mood（2px bob / 1px shake / 1.4s / 無 rotation）後反饋「動的幅度太小、不像 1.1 design」。CDP probe 證實實作 verbatim 對齊 spec、HMR 套上新值都正確——是 **spec 數值本身過於 subtle、在 1920×1080 100% scaling 下接近視覺臨界**。

經 4 輪 user-as-design-team 交互式調整（2px → 4px → 8px → ±50px mirrored dwell-return）後 user 鎖定：鏡像（公車朝左 `scaleX(-1)`）+ ±50px X 軸範圍 + ±2° rotation + bumpy Y (-3px) + 2.5s loop with dwell-return arc。實作已落在 `codebus-app/src/styles/globals.css`，但 spec / AUDIT brand motion lock / LO-3 動畫詞彙表 / ODI-1 archived entry 未同步——本 change 把 spec drift 補上、避免未來 review 對不上現況。

## What Changes

- **Rename**：`Requirement: Lobby Empty State Idle Motion` → `Lobby Empty State Hero Motion`（移除「idle」誤導用詞）
- **Revise**：spec requirement 正文 + 3 個 scenario 從「2px bob + 1px shake + 無 rotation」改寫為「±50px mirrored cyclic + ±2° rotation + -3px bumpy Y + 2.5s dwell-return loop」
- **Revise**：AUDIT brand motion lock（line 540-562）從「2 個合法 mood」改寫——廢除「Idling in place」mood，新增「Moving forward · cyclic」mood（或「Moving forward family + loading/cyclic 變體」架構，apply 階段 design.md 對齊後二選一）
- **Revise**：AUDIT LO-3 動畫詞彙表（line 625-630）二分法更新——`codebus-bus-roll`（單向 loading）= LoadingOverlay、`codebus-bus-roll-mirrored`（mirrored cyclic ambient）= 04b hero
- **Append**：AUDIT ODI-1 archived entry（line 1827-1837）保留 archived 本體、補 2026-05-28 revision footnote 紀錄 user-as-design override 決定
- **Revise**：`codebus-app/src/styles/globals.css` line 111-113 comment 從「TEMP experiment」改寫為正式 spec 引用
- **Remove**：`@keyframes codebus-bus-idle-y` / `codebus-bus-idle-x`（grep 確認 0 active consumer、僅 archive 文檔引用）

不破壞既有 hard nos（Wordmark 🚌 不動 / Goal Running 不加 bus / Quiz generation 不加 bus / `prefers-reduced-motion: reduce` 完全靜態）；不動 LoadingOverlay 的 `codebus-bus-roll` keyframe。

## Non-Goals

- **不動 LoadingOverlay**：`codebus-bus-roll` keyframe（globals.css line 92-109）+ `LoadingOverlay.tsx` consumer 不在本 change scope，cyclic mirrored 變體只用於 04b empty hero。
- **不重造 mood disambiguation 概念**：apply 階段 design.md 選定「3 mood 並列」OR「family + variant」架構之一、不兩個都寫。
- **不擴張 bus motion 適用面**：Goal Running / Quiz generation / Wordmark 既有 hard no 全保留。
- **不收 LO-2 標題文案 / LO-4 wall-clock wording**：那是 LoadingOverlay 議題、與本 change 無關。
- **不動 motion magnitude 以外的 user lock 數值**：±50px / 2.5s / scaleX(-1) / dwell-return arc 已 user-as-design 鎖定、本 change 只把 spec drift 補齊、不重啟調參。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-shell`: `Lobby Empty State Idle Motion` requirement 改名為 `Lobby Empty State Hero Motion`、正文與 3 個 scenario 重寫為新 motion spec（±50px mirrored cyclic + ±2° rotate + -3px bumpy Y + 2.5s dwell-return）

## Impact

- Affected specs: `app-shell`（Lobby Empty State Hero Motion requirement 重寫）
- Affected code:
  - Modified:
    - openspec/specs/app-shell/spec.md
    - codebus-app/design-handoff/AUDIT.md
    - codebus-app/src/styles/globals.css
  - Removed:
    - codebus-app/src/styles/globals.css 內 `@keyframes codebus-bus-idle-y` / `codebus-bus-idle-x`（grep 確認 0 active consumer）
- Affected tests:
  - codebus-app/src/components/lobby/EmptyState.test.tsx（grep 確認只斷言 `.codebus-bus-idle` class、未斷言 keyframe name / 數值；無須改、但 apply 階段重 grep 再確認）
- 跨文件同名詞 disambiguation 風險：「mood」概念跨 spec + AUDIT brand lock + AUDIT LO-3 vocab 三段重寫、apply 階段必須一次校齊、不可只改其一造成新一輪 drift
