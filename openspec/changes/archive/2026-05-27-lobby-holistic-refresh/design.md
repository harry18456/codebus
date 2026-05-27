## Context

#### Pre-apply 校準（trailer 寫的 vs 現況實機差異）

跟 trailer 寫的範圍對照 `codebus-app/src/components/lobby/*` 與 `codebus-app/src/i18n/messages.ts` 實機，校準如下，apply 時依此版本為準：

- **EmptyState 行號**：AUDIT 寫 `EmptyState.tsx:35`（quickstart card）/ `EmptyState.tsx:48`（step number）；實機是 line 36 / line 49。本 change 用元素語意（quickstart card、step number `<span>`）描述、不依賴行號。
- **G6 footer 缺頂線 + sunken bg**：AUDIT 要求補 `border-top` + `bg-sunken`；現況 `codebus-app/src/components/BottomStrip.tsx` 已套 `border-t border-border bg-bg-sunken`。G6 樣式本體不再動；只剩 Lobby `<main>` layout（G1）讓內容不再垂直置中、footer 自然黏底（footer 由 App.tsx 渲染，跟 Lobby 是 sibling）。
- **G-copy-1 副標 / step wording**：AUDIT 寫「`lobby.empty.step2` → 想一個想搞懂的問題（goal）—例如『auth 怎麼運作』」並標 "Phase 3A 之前 landed"；但實機 `lobby.empty.step2` zh 仍是「跑一個 goal — 例如「搞懂這 repo 的 X」」，en 同樣含「搞懂這 repo 的 X」中英混雜。本 change 明確不動 G-copy-1 wording（per trailer "不要的事"）；G2 amber pill 包現有 step2 example wording，pill 本身與 wording 無耦合，未來 G-copy-1 wording 真正 land 時 pill 樣式不需要再改。
- **i18n key 新增 3 個（`lobby.topbar.addAction` / `lobby.section.recent` / `lobby.dragTip.text`）**：trailer 列為要加；AUDIT G-copy-2 line 477 明寫「key 命名不動，只改 value」；現況已存在 `lobby.topbar.newVaultButton` / `lobby.populated.sectionLabel` / `lobby.populated.dragTip`。決議保留既存 3 個 key、只改 zh + en value（已跟 user 在 propose 階段 confirm）。
- **`vault` 字面 grep**：實機 i18n bundle 有 vault 字面的 key 共 6 處——`lobby.topbar.newVaultButton`（zh `+ 新增 Vault` / en `+ New Vault`）、`lobby.populated.sectionLabel`（zh `近期 Vault` / en `Recent vaults`）、`lobby.populated.dragTip`（zh `成新 vault` / en `open it as a vault.`）、`lobby.empty.cta`（zh `+ 搭一台新公車` / en `+ Board a new bus`，cta 是 bus 不是 vault 字面，本 change 不動）、`lobby.empty.title`（同樣 bus 不是 vault）、`lobby.empty.subtitle`（zh / en 都含 `repo` 但無 `vault`，G-copy-1 範圍）。本 change 只動 G-copy-2 範圍的 3 個 key（topbar / sectionLabel / dragTip）。

#### Phase 4A 在 Phase 序列中的位置

依 AUDIT `Phase 4A · lobby-holistic-refresh` 段落，scope 集中在 `codebus-app/src/components/lobby/*` 跟少量 `codebus-app/src/i18n/messages.ts` 與 `codebus-app/src/styles/globals.css`。Phase 4B（Workspace sidebar）、Phase 4C（Goals / Quiz tabs）有自己的 change，不在本 change。`SectionLabel` 元件在 Phase 2 已 build、由 `codebus-app/src/components/ui/SectionLabel.tsx` 提供 `variant="caps" | "default"`，本 change 直接 import。

## Goals / Non-Goals

**Goals:**

- **G1 layout fix**：Lobby `<main>` 改成 `flex flex-col` + Quickstart 內容自然向上對齊，empty / populated 兩態都不再有「viewport 幾何置中」造成的上下大留白感
- **G2 amber pill**：04b Quickstart step2 example 部分包成 amber-tinted mono pill；pill 樣式對齊 `design_files/styles.css` 的 `.cb-qs-quote` token（mono / `bg-accent-tint` / `text-accent` / 細 padding / `border-radius: 3px` / 1px amber-tinted border）
- **G3 step number style**：步驟編號從 `1.` `2.` `3.` 改 mono 數字無句點配 `text-fg-tertiary`，視覺收斂
- **G7 density**：Quickstart card 內部步驟間距收緊，跟 G3 一起調
- **G4 section label 套 SectionLabel**：04a「近期」與 04b「快速開始」改用 `<SectionLabel>` 元件，中英文視覺一致
- **G-copy-2 UI 拿掉 vault**：3 個 i18n key value 改成不含 vault 字面的新 wording（zh + en 各一遍）
- **G-04a-1 kebab 互動**：VaultCard 加可見 kebab 按鈕（hover 顯示）開 menu，保留 `onContextMenu` right-click 當 shortcut
- **ODI-1 idle motion**：04b hero 56px 🚌 加 idle micro-motion（2px vertical bob + 1px horizontal jitter，1.4s loop，純 CSS keyframes，`prefers-reduced-motion: reduce` 完全靜態 fallback）

**Non-Goals:**

- 不動 Workspace、Goals tab、Quiz tab、Wiki tab
- 不重新命名既存 i18n key（per AUDIT G-copy-2 line 477）
- 不改 G-copy-1 範圍 wording（`lobby.empty.title` / `lobby.empty.subtitle` / `lobby.empty.cta` / `lobby.empty.step1-3` 的整段 prefix wording）；G2 pill 只負責包 step2 example 部分、不負責改 wording
- 不動 `vault` 字面在 CLI / `VaultEntry` data model / README / config YAML 的使用
- 不動 G6 footer 樣式（已 land）；只調 Lobby `<main>` layout 讓 footer 在 viewport 自然黏底
- 不做 ODI-2 fullscreen ambient background（備案）
- 不做大幅 transform / opacity / 路面 / 輪子動畫；motion 必須是 subtle、不干擾閱讀

## Decisions

### Lobby `<main>` 從 vertical-center 改 flex-column top-aligned

把 Lobby 主元件內的 wrapper 從 `flex flex-1 w-full items-center justify-center px-6 py-8` 改成 `flex flex-1 w-full flex-col items-center px-6 py-8` 並加 top padding 讓內容自然上排。BottomStrip 在 App.tsx 是 Lobby 的 sibling、本來就在底端，layout 改完內容上排後 viewport 底部留給 BottomStrip 自然黏底感。empty 跟 populated 共用同一個 layout 變更（兩態都受 G1 影響）。

**Alternative**：只改 empty 不改 populated → 兩態切換時 layout 跳動且 populated 也有同樣大留白問題（AUDIT 451-462 已記錄）→ 不採用。

### Quickstart card 改用 grid 編排步驟

EmptyState 的 `<ol>` 從 `space-y-2` 改成 grid（22px column for step number、1fr for text）對齊 design v1 styles 的 `.cb-quickstart-steps` 規格（`grid-template-columns: 22px 1fr`），步驟編號 `<span>` 改成 mono / `text-fg-tertiary` / 無句點。先取 mono 數字無 22×22 box，box 形式跟 design v1 對齊後若需要再升級。

**Alternative**：保留 `space-y-2` 只改 number 樣式 → 視覺密度仍不收斂（G7 跟 G3 是連動 gap）→ 不採用。

### G2 amber pill 採 split-key 渲染（step2 拆 step2Prefix + step2Example）

新增 `lobby.empty.step2Example` i18n key 存 example 部分 wording、`lobby.empty.step2` 保留為 prefix wording（不含 example），EmptyState 渲染 step2 時拼接 `prefix + <span className="amber-pill...">{example}</span>`。pill 樣式直接寫 Tailwind utility（`bg-accent-tint text-accent border border-accent/20 rounded-sm px-1.5 py-px font-mono text-meta`），對齊 design v1 styles 的 `.cb-qs-quote`。

**Alternative 1**：保留 step2 為單一 key、用字串切割（split on 「」/ '"'）切出 example 包進 pill → 依賴特定引號字元、locale switching 時需保證 zh 跟 en value 都用相同引號樣式 → 不採用。

**Alternative 2**：i18n value 直接內嵌 JSX → useT 不支援 JSX node、且 i18n bundle 不該耦合 component tree → 不採用。

理由：split-key 在 codebus i18n bundle 已有 prefix/suffix 分段 convention，新增 key 在 G-copy-1 範圍外、不違反「不動 G-copy-1 wording」（key 是技術 identifier、不是 wording 改動）；future G-copy-1 wording 真正 land 時換 `step2Example` value 即可，pill 樣式不動。

### G4 section label 換 SectionLabel 元件

04a populated 從 `text-fg-tertiary text-micro font-semibold uppercase tracking-[0.12em]` 樣式改 `<SectionLabel>` default variant（無 uppercase tracking）；section label 旁的 vault count 從目前獨立 `<div>` 整合進 `<SectionLabel count={vaults.length}>`。04b empty 的 `lobby.empty.quickstartLabel`（QUICKSTART / 快速開始）同樣改 `<SectionLabel>` default variant。符合 AUDIT G4 line 412-420 的 14 位置 sweep 決議（Lobby populated「最近」、Lobby empty「快速開始」用 default variant，不是 caps）。

**注意**：trailer 寫「套 `<SectionLabel variant="caps">`」是 prompt 端的疏失，AUDIT 第 412-420 行明示這兩個位置走 default variant、不是 caps；caps variant 是給 Wiki tree 5 buckets 英文 caps 使用。本 change 採 AUDIT 版本（default variant）。

**Alternative**：保留 `uppercase tracking-[0.12em]` 自行寫 div → AUDIT G4 line 369 已明示中文 fragment 不吃 tracking 是 root cause → 不採用。

### G-04a-1 VaultCard 加 hover-revealed kebab

VaultCard 加一個 kebab `<button>` 放在 right-aligned 位置、`opacity-0 group-hover:opacity-100 focus-visible:opacity-100` 控制可見性（card root 已是 `group`），click handler 走跟現有 `onContextMenu` 一樣的 menu open 邏輯（複用 menuOpen / menuPos state），menu position 改 anchored 到 kebab button 而非 mouse position（kebab click 走 anchored、right-click 走 mouse position）。menu items 不變、`vaultCard.menu.revealInFiles` / `vaultCard.menu.remove` 既存 i18n key 沿用。kebab icon 用 `lucide-react` 的 `MoreVertical` 維持 codebus-app icon system 一致。

**Alternative**：永遠可見 kebab → 視覺雜訊大、設計稿明示是 hover-revealed → 不採用。

### ODI-1 idle motion 純 CSS keyframes

在 globals 樣式表加 `@keyframes codebus-bus-idle-y`（vertical 2px translate）+ `@keyframes codebus-bus-idle-x`（horizontal 1px translate）+ `.codebus-bus-idle` selector 套兩條 animation（y 走 1.4s、x 走 1.1s，desynced ease-in-out infinite），不加 rotation、不改 opacity。EmptyState hero `<div>` 加 `codebus-bus-idle` className。`@media (prefers-reduced-motion: reduce)` rule 將 `.codebus-bus-idle` animation 改 `none`。跟既有 `codebus-bus-roll`（LoadingOverlay）共用 keyframe naming convention（`codebus-bus-{kind}`）。

**Alternative**：用 framer-motion → 加外部依賴、無必要 → 不採用。

## Implementation Contract

#### Behavior

實作完成後，從 user 視角觀察到：

- **Lobby empty (04b)**：
  - 內容（hero 🚌 + title + subtitle + CTA + Quickstart card）自然向上排，1920×1080 100% 縮放下 viewport 上半部填滿、底部留給 BottomStrip 黏底，不再有大塊上下空白
  - Quickstart card 內 step2 的 example wording 部分有 amber-tinted mono pill 視覺
  - 步驟編號為 mono 數字、無句點、用 `text-fg-tertiary` 色
  - Quickstart card 內步驟間距較目前緊
  - 快速開始 / `QUICKSTART` section label 用 `<SectionLabel>` 元件（default variant），中英文視覺一致
  - 🚌 hero 在 viewport 內持續做 subtle 上下 + 左右 micro motion，1.1-1.4s desynced loop；開啟系統 `prefers-reduced-motion` 後 motion 立即停止
- **Lobby populated (04a)**：
  - 最近 / `Recent` section label 用 `<SectionLabel>` 元件（default variant），vault 字面從 UI 消失（zh 顯示「最近」、en 顯示「Recent」）
  - topbar `+ 新增` 按鈕（zh）/ `+ Add`（en）取代 `+ 新增 Vault` / `+ New Vault`
  - drag tip 文字不含 vault 字面（新 wording 在 i18n value）
  - VaultCard 在 hover 時右側出現可見 `⋮` kebab 按鈕、click 開出 menu（Reveal in files / Remove）；right-click 在 card 任意位置也開同樣 menu，是 shortcut
  - 內容自然向上排（跟 empty 共用 layout）

#### Interface / data shape

- i18n keys（所有在 codebus-app 的 messages 檔）：
  - `lobby.topbar.newVaultButton`：保留 key，zh value 改 `+ 新增`、en value 改 `+ Add`
  - `lobby.populated.sectionLabel`：保留 key，zh value 改 `最近`、en value 改 `Recent`
  - `lobby.populated.dragTip`：保留 key，zh value 改成不含 vault 字面的 wording（原則：保留 drag-to-add 動作描述、用「程式碼資料夾 / folder」描述對象，跟 G-copy-1 副標的「程式碼資料夾」用詞保持一致），en value 對應改
  - `lobby.empty.step2`：保留 key、value 改為 prefix-only（zh 取自原 value「跑一個 goal — 例如」/ en 取自原 value `Run a goal — e.g.`）
  - `lobby.empty.step2Example`：新增 key，zh value 取自原 step2 引號內 wording（`搞懂這 repo 的 X`）、en value 同字串（G-copy-1 wording 不在本 change 範圍）
- 元件：
  - EmptyState：渲染 Quickstart step2 時把 `t("lobby.empty.step2")` 當 prefix + `<span className="amber-pill...">{t("lobby.empty.step2Example")}</span>` 拼接
  - Lobby：`<main>` 內 wrapper layout 改 `flex-col`、移除 `items-center justify-center`
  - VaultCard：root 已是 `group`，新增 kebab `<button>` 顯示控制走 `opacity-0 group-hover:opacity-100 focus-visible:opacity-100`、click handler 設 `menuPos` anchored 到 button bounding rect
- 樣式：
  - globals 樣式表新增 `@keyframes codebus-bus-idle-y` + `@keyframes codebus-bus-idle-x` + `.codebus-bus-idle` selector + `@media (prefers-reduced-motion: reduce)` fallback
  - EmptyState hero 容器加 `codebus-bus-idle` className

#### Failure modes

- i18n value 改動後若舊版本快取 / persisted state 含舊 key value：i18n 是 build-time bundle、無 persistence 風險
- `prefers-reduced-motion` 環境：CSS `@media` 自動降級為靜態，無 JS 介入
- VaultCard kebab 在無 hover 環境（touch / 鍵盤）：`focus-visible:opacity-100` 確保鍵盤 navigation 時可見並可被 Enter 觸發；touch 環境降級走 long-press（瀏覽器預設）或直接點 card → 不打開 menu（這是現況、不變）

#### Acceptance criteria

- `pnpm tsc` 與 `pnpm test` 兩個 codebus-app 既存指令必須綠
- 真實 CDP smoke（透過 `codebus-app/scripts/cdp.mjs` 連 WebView2）兩種 locale 各跑一遍：
  - **04b empty smoke**：清空 vault list 後，截圖驗 (a) Lobby 內容不再 viewport 垂直置中、上排對齊、底部 BottomStrip 自然黏底；(b) Quickstart step2 example 有 amber pill；(c) 步驟編號 mono 無句點；(d) 「快速開始」/`QUICKSTART` 用 SectionLabel 元件渲染（DOM 含 `.section-label` class）；(e) 🚌 hero 有 idle micro-motion（截圖兩次間隔 0.4s 應看到 transform 變化，或讀 DOM `getComputedStyle` 確認 `animation-name` 含 `codebus-bus-idle-y`）
  - **04a populated smoke**：建 1-3 個測試 vault 後，截圖驗 (a) section label 顯示「最近」/`Recent`、無 vault 字面；(b) topbar CTA 無 `+ 新增 Vault`/`+ New Vault` 字面；(c) drag tip 無 vault 字面；(d) VaultCard hover 時 kebab 可見、click 開 menu；(e) right-click 仍可開 menu
  - **prefers-reduced-motion smoke**：CDP 開 `emulateMedia` reduced-motion，截圖驗 🚌 hero `animation-name: none`
- 截圖全部存進 `codebus-app/scripts/.lobby-refresh-smoke/` 目錄、命名含 locale + 場景 + 步驟
- 視覺比對 design v1 lobby reference（`codebus-app/design-handoff/design_files/components/lobby.jsx` + `codebus-app/design-handoff/design_files/styles.css`），主要 diff 限定在 G-copy-1 範圍 wording 與 G5 邊框對比（兩者本 change 不負責）
- spec delta 通過 spectra validate strict 模式

#### Scope boundaries

- **In scope**：
  - codebus-app 的 Lobby 元件（Lobby.tsx）、EmptyState 元件、VaultCard 元件
  - codebus-app 的 i18n messages 上述 5 個 key 的 value 修改與 1 個新 key（`lobby.empty.step2Example`）
  - codebus-app 的 globals 樣式表新增 idle keyframes 與 selector
  - app-shell spec 的 Lobby 相關 requirements delta（wording 微調與 Quickstart pill / kebab affordance / idle motion 三項補述）
  - CDP smoke 截圖目錄 `codebus-app/scripts/.lobby-refresh-smoke/`
- **Out of scope**：
  - Workspace、GoalsTab、QuizTab、WikiTab、ChatWidget、LoadingOverlay、BottomStrip、WindowControls、SettingsModal、NewVaultFlow 元件
  - CLI vault 相關指令與 `VaultEntry` Rust struct / TS type
  - G-copy-1 範圍 wording（title / subtitle / cta / step1-3 prefix）
  - Phase 2 token bump（已 land）、Phase 4B/4C 的 sidebar 與 tabs 改動

## Risks / Trade-offs

- **[Risk] split-key 渲染對未來 G-copy-1 wording 改動的影響** → Mitigation：G-copy-1 真正 land 時若 step2 wording 整段改寫成「想一個想搞懂的問題（goal）— 例如『auth 怎麼運作』」，把 `step2` value 換成新 prefix、`step2Example` 換成新 example，pill 樣式不需要動；split-key 反而讓 G-copy-1 future change 更乾淨
- **[Risk] kebab `group-hover:opacity-100` 在 keyboard-only navigation 看不見** → Mitigation：加 `focus-visible:opacity-100`、確保 Tab 進 VaultCard 時 kebab 可見並可被 Enter 觸發
- **[Risk] idle motion 在低階 GPU 環境 jank** → Mitigation：純 `transform` translate（非 layout-trigger），1-2px 範圍幾乎無 GPU 壓力；prefers-reduced-motion fallback 兜底
- **[Risk] AUDIT 行號跟實機 off-by-1 / 過時 description（如 G6）apply 時繼續踩** → Mitigation：本 design.md「Pre-apply 校準」段已記下、tasks 用元素語意而非行號定 site；apply 第一步仍按 trailer 要求做 grep 再校準一輪
- **[Risk] trailer 寫「`<SectionLabel variant="caps">`」與 AUDIT G4 sweep 表決議（default variant）矛盾** → Mitigation：本 change 採 AUDIT 版本（default variant），design.md 明記、tasks 用 default variant；caps variant 留給 Wiki tree 5 buckets

## Migration Plan

Solo dev 模式直接 main，無 feature branch、無 rollback strategy 需求。實作完成後跑驗收四步（`pnpm tsc` / `pnpm test` / 兩 locale CDP smoke / 視覺比對 design v1），通過後直接 commit 進 main。

## Open Questions

- drag tip 新 wording 具體用「程式碼資料夾」還是「資料夾」？（zh 對應）→ 在 tasks 階段定，原則：跟 G-copy-1 副標的「程式碼資料夾」用詞保持一致
- ODI-1 motion duration 用 1.4s 還是 1.6s loop？AUDIT 寫「1.2-1.6s」彈性範圍 → 預設 1.4s（y 軸）/ 1.1s（x 軸 desync），CDP smoke 後若感覺太頻繁再調慢
