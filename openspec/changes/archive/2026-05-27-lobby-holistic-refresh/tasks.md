<!--
每個 task 描述「完成時實際可觀察到什麼」+「如何驗證」。
檔案路徑只是定位 context、不是 task 本身。
parallel_tasks: true → 跨檔案、跨 group 無 dependency 的 task 標 `[P]`。
-->

## 1. Pre-apply 校準（trailer Task 1.1）

- [x] 1.1 重新 grep `codebus-app/src/components/lobby/` 目錄底下的 Lobby.tsx / EmptyState.tsx / VaultCard.tsx 三個檔，逐項驗 G1 / G2 / G3 / G4 / G6 / G7 / G-04a-1 / G-copy-2 / ODI-1 在實機是否仍存在；驗證方法：對照 design.md「Pre-apply 校準」段表格逐項勾選，每一項用元素語意（不是行號）描述當前狀態。任何 site 已被別的 change 順手 land、或修法描述跟實機脫節，必須在本 tasks.md 開頭加一條「校準補述」筆記、不要默默偷渡修改。
- [x] 1.2 grep `codebus-app/src/i18n/messages.ts` 確認本 change 要動的 5 個 i18n key (`lobby.topbar.newVaultButton` / `lobby.populated.sectionLabel` / `lobby.populated.dragTip` / `lobby.empty.step2`) 與 1 個新 key (`lobby.empty.step2Example`) 不會跟其他 namespace 撞名；驗證方法：`grep -n 'lobby.empty.step2Example' codebus-app/src` 應 0 hit、其他四個 key 各應該 zh + en 兩處（兩個 message map 各一）。

## 2. i18n value 更新（G-copy-2 + step2 split-key）

- [x] 2.1 [P] **Lobby Two-State Rendering** 的 topbar add-action 文字實際渲染為不含 "Vault"/"vault" 字面：把 `lobby.topbar.newVaultButton` 的 zh value 改為 `+ 新增`、en value 改為 `+ Add`；驗證方法：`codebus-app/src/i18n/messages.ts` 兩個 map 對應 key 的 value 各無 "Vault"/"vault" 子字串。
- [x] 2.2 [P] **Lobby Two-State Rendering** 的 populated section label 渲染為不含 "Vault"/"vault" 字面：把 `lobby.populated.sectionLabel` 的 zh value 改為 `最近`、en value 改為 `Recent`；驗證方法：同上 grep 規則對應 key 無 vault 字面。
- [x] 2.3 [P] **Lobby Two-State Rendering** 的 drag tip 渲染為不含 "Vault"/"vault" 字面：把 `lobby.populated.dragTip` 的 zh value 改為「提示・把程式碼資料夾拖進這個視窗就能加入清單。」（對齊 G-copy-1 副標「程式碼資料夾」用詞），en value 改為 `tip · Drag a code folder anywhere into this window to add it to the list.`；驗證方法：兩 value 無 vault 字面、保留 drag-to-add 動作描述。
- [x] 2.4 **G2 amber pill 採 split-key 渲染（step2 拆 step2Prefix + step2Example）**：把 `lobby.empty.step2` 改為「step2 prefix-only」（zh value 改為 `跑一個 goal — 例如`、en value 改為 `Run a goal — e.g.`），新增 `lobby.empty.step2Example` key（zh value 與 en value 同字串 `搞懂這 repo 的 X`，遵守「G-copy-1 wording 不在本 change 範圍」原則）；驗證方法：`pnpm test` 跑既存 lobby i18n test 仍通過，新 key `lobby.empty.step2Example` 在兩 message map 都有對應 value、且 prefix value 不再包含原引號內 example wording。

## 3. Lobby `<main>` layout（G1，**Lobby `<main>` 從 vertical-center 改 flex-column top-aligned**）

- [x] 3.1 **Lobby Two-State Rendering** 「Lobby content flows from the top」scenario 在 empty 與 populated 兩態都成立：把 `codebus-app/src/components/lobby/Lobby.tsx` 內 `<main>` 子層 wrapper（現況 `flex flex-1 w-full items-center justify-center px-6 py-8`）改為 `flex flex-1 w-full flex-col items-center px-6 py-8` 並加合適 top padding；驗證方法：CDP smoke 1920×1080 100% 縮放下截圖兩態，hero/cards 對齊 viewport 上半部、不再幾何置中、底部由 BottomStrip sibling 占據。

## 4. Quickstart card 內部編排（G3 + G7，**Quickstart card 改用 grid 編排步驟**）

- [x] 4.1 **Lobby Two-State Rendering** 「Quickstart step number uses monospace digits without period」scenario 成立：把 `codebus-app/src/components/lobby/EmptyState.tsx` 內 step list 的 `<ol>` + `<li>` 樣式改為 grid（`grid grid-cols-[22px_1fr]` 或等價 utility 對齊 design v1 styles `.cb-quickstart-steps` 的 22px column），步驟編號 `<span>` 改為 mono 數字無句點配 `text-fg-tertiary`；驗證方法：DOM inspect / 截圖無 `1.` `2.` `3.` 句點、編號 monospace 字體呈現。
- [x] 4.2 [P] Quickstart card 步驟間距收緊（G7）：把 `<ol>` 的 vertical gap 從 `space-y-2` 改為較緊的 grid gap-y（對齊 design v1 styles `.cb-quickstart-steps` 的 `gap: 8px`），整張 card padding 維持 14/18；驗證方法：CDP smoke 截圖跟 G3 一起驗整體密度收斂、不再「鬆散」。

## 5. Quickstart step 2 amber pill（G2，**G2 amber pill 採 split-key 渲染（step2 拆 step2Prefix + step2Example）**）

- [x] 5.1 **Lobby Two-State Rendering** 「Quickstart step 2 example renders in amber pill」scenario 成立：在 `codebus-app/src/components/lobby/EmptyState.tsx` 內把 step2 渲染從單一 `{t(key)}` 改為「`{t("lobby.empty.step2")}` + 空格 + `<span className="...">{t("lobby.empty.step2Example")}</span>`」，pill `<span>` 樣式為 `bg-accent-tint text-accent border border-accent/20 rounded-sm px-1.5 py-px font-mono text-meta`（對齊 design v1 styles `.cb-qs-quote`）；驗證方法：CDP smoke 截圖 step2 example 文字呈 amber pill 視覺；step1/step3 樣式不受影響。

## 6. Section label 套 SectionLabel（G4，**G4 section label 換 SectionLabel 元件**）

- [x] 6.1 **Lobby Two-State Rendering** 「Section labels use the shared SectionLabel component」scenario 在 populated 成立：把 `codebus-app/src/components/lobby/Lobby.tsx` 內 PopulatedList 的 section label `<div className="text-fg-tertiary text-micro font-semibold uppercase tracking-[0.12em]">` 與相鄰 count `<div>` 替換為 `<SectionLabel count={vaults.length}>{t("lobby.populated.sectionLabel")}</SectionLabel>` 從 `@/components/ui/SectionLabel` import；驗證方法：DOM 含 `.section-label` class、無 `uppercase tracking-[0.12em]` 在這個位置；CDP smoke 截圖中文「最近」label 跟 vault count 視覺一致。
- [x] 6.2 [P] **Lobby Two-State Rendering** 「Section labels use the shared SectionLabel component」scenario 在 empty Quickstart 成立：把 `codebus-app/src/components/lobby/EmptyState.tsx` 的 Quickstart label `<div className="text-fg-tertiary text-micro font-semibold uppercase tracking-[0.12em]">{t("lobby.empty.quickstartLabel")}</div>` 替換為 `<SectionLabel>{t("lobby.empty.quickstartLabel")}</SectionLabel>` 預設 variant；驗證方法：DOM 含 `.section-label` class、`快速開始` / `QUICKSTART` 視覺一致。

## 7. VaultCard kebab affordance（G-04a-1，**G-04a-1 VaultCard 加 hover-revealed kebab**）

- [x] 7.1 **Lobby Two-State Rendering** 「Vault card kebab visible on hover and focus」與「Vault card kebab hidden when idle」與「Vault card right-click still opens menu」三個 scenario 同時成立：在 `codebus-app/src/components/lobby/VaultCard.tsx` 加一個 kebab `<button>`（icon 用 `lucide-react` 的 `MoreVertical`），className 含 `opacity-0 group-hover:opacity-100 focus-visible:opacity-100 transition-opacity`，click handler 設 `menuPos` 為 button `getBoundingClientRect()` 對應位置並 open menu；保留現有 `onContextMenu` 走 mouse position 開 menu 的行為；驗證方法：CDP smoke 在 populated 截圖 (a) idle 狀態 kebab 不可見、(b) hover 狀態可見、(c) click kebab 開 menu、(d) right-click card 任意位置仍開 menu；vitest 加單元測試覆蓋「focus-visible 時 kebab 顯示」+「click kebab 開啟 menu」兩條路徑。

## 8. ODI-1 hero idle motion（**ODI-1 idle motion 純 CSS keyframes**）

- [x] 8.1 [P] **Lobby Empty State Idle Motion**「Hero 🚌 animates with idle micro-motion by default」與「Idle motion is scoped to empty-state hero」兩個 scenario 成立：在 `codebus-app/src/styles/globals.css` 新增 `@keyframes codebus-bus-idle-y`（垂直 0 → -2px → 0 translate）+ `@keyframes codebus-bus-idle-x`（水平 0 → 1px → 0 translate）+ `.codebus-bus-idle` selector 套 `animation: codebus-bus-idle-y 1.4s ease-in-out infinite, codebus-bus-idle-x 1.1s ease-in-out infinite`，不加 rotation、不改 opacity；在 `codebus-app/src/components/lobby/EmptyState.tsx` 的 hero `<div className="text-[56px]" aria-hidden="true">🚌</div>` 加 `codebus-bus-idle` className；topbar 🚌 wordmark 不加；驗證方法：CDP smoke 在 empty 截圖兩次間隔 400ms 應觀察到 hero transform 變化，或 `getComputedStyle` 讀到 `animation-name` 含 `codebus-bus-idle-y`；topbar 🚌 對應 element 的 `animation-name` 應為 `none`。
- [x] 8.2 **Lobby Empty State Idle Motion**「Reduced-motion preference disables idle motion」scenario 成立：在 `codebus-app/src/styles/globals.css` 加 `@media (prefers-reduced-motion: reduce) { .codebus-bus-idle { animation: none; } }` rule；驗證方法：CDP smoke 透過 `Emulation.setEmulatedMedia` 設 `prefers-reduced-motion: reduce` 後讀 hero `getComputedStyle` `animation-name` 應為 `none`；同場景截圖 hero 完全靜態。

## 9. 測試覆蓋更新

- [x] 9.1 既存 `codebus-app/src/components/lobby/Lobby.test.tsx` / `EmptyState.test.tsx` / `VaultCard.test.tsx` 重跑全綠：必要時更新 snapshot / 對 i18n value 的斷言、補上 step2 split-key 渲染斷言、補上 kebab focus-visible 斷言；驗證方法：`pnpm test` 在 codebus-app 全綠、無 .skip / xfail 增加。
- [x] 9.2 [P] type check 全綠：跑 `pnpm tsc` 不能新增 type error；驗證方法：CI 等價指令本機回傳 exit code 0。

## 10. CDP smoke 驗收（真實前端，不是 unit test 就算完）

- [x] 10.1 **04b empty smoke**：用 `codebus-app/scripts/cdp.mjs` 連 WebView2，清空 vault list 後分別在 zh / en locale 各跑一遍，截圖存 `codebus-app/scripts/.lobby-refresh-smoke/04b-{zh|en}-{step}-*.png`；驗證 G1 footer 黏底 / G2 amber pill / G3 mono 編號 / G7 密度 / G4「快速開始」走 SectionLabel default variant / ODI-1 🚌 idle motion 有動。
- [x] 10.2 **04a populated smoke**：建 1-3 個測試 vault 後分別在 zh / en locale 各跑一遍，截圖存 `codebus-app/scripts/.lobby-refresh-smoke/04a-{zh|en}-{step}-*.png`；驗證 G4「最近」走 SectionLabel default variant（無 vault 字面）/ VaultCard 有 kebab hover affordance / drag tip 無 vault 字面 / topbar CTA 無 `+ 新增 Vault` 或 `+ New Vault` 字面。
- [x] 10.3 [P] **prefers-reduced-motion smoke**：透過 `Emulation.setEmulatedMedia` 設 `prefers-reduced-motion: reduce` 後在 empty state 截圖、讀 hero `animation-name`；截圖存 `codebus-app/scripts/.lobby-refresh-smoke/reduced-motion-hero.png`；驗證 hero `animation-name` 為 `none`。
- [x] 10.4 視覺比對 design v1 reference：把上述四組截圖跟 `codebus-app/design-handoff/design_files/components/lobby.jsx` + `codebus-app/design-handoff/design_files/styles.css` 並列比對；驗證主要 diff 只剩 G-copy-1 範圍 wording 與 G5 邊框對比兩項（兩者本 change 不負責），其餘視覺對齊 design v1；不符要在 tasks.md 加「校準補述」說明。

## 11. Spec / AUDIT 補述（archive 階段順手做）

- [x] 11.1 archive 階段在 `codebus-app/design-handoff/AUDIT.md` 對應 gap 段（G1 / G2 / G3 / G4 / G6 / G7 / G-04a-1 / G-copy-2 / ODI-1）每條加 `archived 2026-05-27 via lobby-holistic-refresh` 標記；驗證方法：archive 後 grep AUDIT.md 對應 gap heading 同一段含 archived stamp。
