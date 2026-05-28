## Context

Phase 6 v1.1 mock landing 第三塊。Wiki page reader 設計 v1.1 reply（2026-05-26 design v1.1 交付）把 design 灰區 5 個 view 全 spec lock，本 change 把 wiki 相關條目（WP2 / WP5 / WP10 / WP11 / WP-tree-footer / WP-empty-page / WK-EMPTY-1/2/3）一次性落地。

**現況**：

- `codebus-app/src/components/workspace/WikiPreview.tsx` 用 react-markdown 渲染（Milkdown 已 swap、保留 dep 給未來 ProseMirror 遷移）。
- Wikilink 已實作兩 state：resolvable 用 inline 色 + hover underline；unresolvable 用 dimmed span + `pageNotFound` title。
- `codebus-app/src/components/workspace/WikiTab.tsx` 已有 empty state 一行 hint（i18n key `workspace.wiki.empty`）+ 三 button bottom row（Quiz me on this / Open in Obsidian）。
- `codebus-app/src/components/workspace/WikiTree.tsx` 有 5-bucket + OTHER bucket（待解散）。
- Backend `codebus-core/src/wiki/frontmatter.rs` PageFrontmatter 含 title / type (enum singular) / sources (Vec SourceRef) / goals (Vec String) / created / updated / related (Vec String) / stale (bool) 全有；本 change 不動 Rust。
- i18n bundle `codebus-app/src/i18n/messages.ts` 既有 prefix `workspace.wiki.*` + camelCase key（toggleTreeAria / pageNotFound / openInObsidian / quizMeOnThis / empty）。

**規格來源**（按 AUDIT 條目 id 引用，避免 line number 漂移）：

- AUDIT WP2 · Page metadata bar [design v1.1 spec lock]
- AUDIT WP5 · 缺 edit / regenerate action [design v1.1 spec lock]
- AUDIT WP10 · 底部 action button 樣式 + i18n
- AUDIT WP11 · Wikilinks 樣式雙樣式 lock [design v1.1 CSS spec]
- AUDIT WP-tree-footer · Wiki tree footer slot 旅行日誌 [design v1.1 spec lock]
- AUDIT WP-empty-page · Wiki tab 有 page 但未選 page [design v1.1 spec lock]
- AUDIT WK-EMPTY-1 / -2 / -3 · 完全沒 page 的 Wiki tab empty state

對應 mock：`codebus-app/design-handoff/v1.1-mocks.html` § wiki page reader。

## Pre-apply 校準（重要）

**Propose prompt 跟 AUDIT spec lock 系統性脫節，本 change 全套以 AUDIT 為準**。Propose user prompt 描述的版本在 5 個維度跟已 spec lock 的 design v1.1 衝突；2026-05-28 confirm 後一律按 AUDIT spec 走。記錄於此免得 apply 階段被原 prompt 誤導：

| 維度 | Propose prompt 描述 | AUDIT spec lock（本 change 採用） |
|---|---|---|
| Metadata bar 內容 | 7-field（goals pill / updated / sources path list / created / type / related / stale） | WP2：三段 only —— Last updated by goal · time-ago · N sources；明確「禁止加 tags / word count / view count / authors」 |
| N sources 含意 | frontmatter sources 的 path list | WP2：body 內 wikilink count（不是 frontmatter sources）；小於 1 時整段不顯示 |
| 旅行日誌 footer slot 位置 | wiki page reader 底部 | WP-tree-footer：wiki tree 底部（不是 page reader） |
| 旅行日誌 footer slot 內容 | runs_referenced（goal run 引用此 page 的 list） | WP-tree-footer：「旅行日誌」當 system page entry（不是 reference list）。Backend grep runs_referenced 0 hit、不需新 backend |
| Edit hint 哲學 | 「edit in Obsidian / VSCode / 你的編輯器」hint | WP5：「想改這頁？跑一個 goal 跟 codebus 說該怎麼改 →」（codebus 哲學：wiki 是 codebus-managed、user 不直接編輯）；點 link 開 NewGoalModal prefill |
| Wikilink CSS | 三 state（normal / visited / broken） | WP11：兩 variant（plain-wikilink body navigation + cite-link citation provenance）；不引入 visited state |
| WP-empty-page 定義 | wiki .md file 存在但 body 全空（frontmatter only） | WP-empty-page：「Wiki tab 有 page 但未選 page」layout；body-only frontmatter 渲染情境 AUDIT 沒 spec、defer |
| i18n key prefix | wiki.* | 既有 = workspace.wiki.*、camelCase key naming |
| 5-bucket identifier | concepts / entities / modules / processes / synthesis（folder 複數） | frontmatter type 值是 singular 小寫 concept / entity / module / process / synthesis；複數是 folder 名；兩組 identifier 不同層、本 change 不展示 type identifier |

**i18n 例外**：codebus 整體採 snake_case naming，但 i18n key 既有 convention 是 camelCase（toggleTreeAria / pageNotFound / quizMeOnThis）—— 本 change 沿用 camelCase。

## Goals / Non-Goals

**Goals:**

- 落地 WP2 metadata bar 三段：Last updated by goal（可點） · time-ago · N sources，所有資料源用既有 PageFrontmatter field、不動 backend。
- 落地 WP5 edit hint footer：「跑 goal 改」link 開 NewGoalModal prefill。
- 落地 WP10 button polish：Quiz button amber 主色、i18n key 加翻譯。
- 落地 WP11 wikilink 雙 variant CSS：plain-wikilink + cite-link；既有 unresolvable 樣式微調對齊新 CSS variable。
- 落地 WP-tree-footer：WikiTree 底部加旅行日誌 system slot、Wiki Index 移到頂、解散 OTHER bucket。
- 落地 WP-empty-page：未選 page 時 reader pane 顯示「📂 選一頁開始讀」hint card。
- 落地 WK-EMPTY-1/2/3：完全沒 page 時 hero icon + 文案 + amber CTA「跑一個 goal 開始」。
- 真實 CDP smoke 驗 zh + en locale 兩條路徑、避開 cdp-smoke 五雷。

**Non-Goals:**

- 不動 backend codebus-core/src/wiki/frontmatter.rs（既有 field 足夠）。
- 不引入 wikilink visited state（AUDIT WP11 沒 spec、避免自創）。
- 不在 metadata bar 加 tags / word count / view count / authors（WP2 明確禁）。
- 不做 runs_referenced list（需要新 backend、不在本 change scope）。
- 不做 body-only frontmatter 渲染 placeholder（AUDIT 沒 spec、defer）。
- 不動 Milkdown markdown 渲染引擎本身（沿用 react-markdown 路徑）。
- 不擴 wiki file tree 結構（Phase 4B sidebar 範圍）。
- 不譯 5-bucket type identifier（identifier、Cat D）。
- 不動 6.1 RunDetailInterrupted / 6.2 ChatWidget（不同 surface）。
- 不動 wikilink IPC seam（reuse useWikiStore.loadPage）。

## Decisions

### Metadata bar 抽出獨立 component

新增 WikiPageMetadataBar component（路徑 codebus-app/src/components/workspace/WikiPageMetadataBar.tsx），props 收 goalLast / updatedIso / wikilinkCount / onGoalClick，return 一個 div 含三段 span（data-testid `wiki-page-metadata-bar`）。抽出讓 test 可直接 mount 不需整個 WikiPreview tree。

**Alternative**：直接寫進 WikiPreview.tsx body。Rejected：metadata bar 三段邏輯（goalLast extract / time-ago format / wikilink count）較複雜、抽出後 test surface 乾淨；WikiPreview.tsx 已 320 lines、繼續堆會超過 codebus 400-line guideline。

### N sources 等於 body wikilink count（不是 frontmatter sources）

per AUDIT WP2 spec lock：N sources 是「body 內 wikilink count」、不是 frontmatter sources 的 path list 長度。

- 實作：reuse 既有 transformBodyWikilinks(body) 回傳的 slugs.length。
- 小於 1 時整段（不只 N、整段「· N sources」）不顯示。

**Alternative**：用 frontmatter.sources.length 顯示「N source code refs」。Rejected：跟 WP2 spec lock 不一致；source code refs 對 user 來說價值低（已 sha256 / at_commit、是 codebus internal provenance），body wikilink count 是 user-facing「這頁引了多少其他 wiki」更直觀。

### Edit hint「跑 goal 改」走既有 NewGoalModal seam

點「跑一個 goal」link → emit 一個 callback onRequestNewGoal(prefilledGoalText) 給 WikiTab 上層 Workspace、由 Workspace 開既有 NewGoalModal 並 prefill。

- Prefill 文案：`修改 wiki/<page-path>.md — `（注意尾巴有「 — 」+ 一個空格、不放結尾、user 自己接「描述你想改的地方」）。
- page-path 取 useWikiStore.pages[currentPath] 的 path 欄位（slug + 5-bucket folder 合成）；apply 階段 grep WikiPageMeta shape 確認 path 欄位存在。

**Alternative**：直接 open external editor。Rejected：跟 WP5 哲學衝突（WP5：人不直接編輯 wiki、起新 goal 改）；codebus 哲學一致性比「快路徑」重要。

### Wikilink 雙 variant CSS 用 Tailwind v4 utility composition + 保留字面 class name

WP11 提供的 CSS spec 用 raw CSS class（plain-wikilink / cite-link），但 codebus 既有 wikilink 用 Tailwind class 加 inline style。本 change 改成：

- plain-wikilink 對應 Tailwind utility composition：`text-fg underline decoration-border-strong underline-offset-[3px] hover:text-accent hover:decoration-accent transition-colors motion-reduce:transition-none`
- cite-link 對應：`font-mono text-accent underline decoration-dashed underline-offset-[3px]`
- 為了讓 test 仍能 query 樣式類別，**保留** plain-wikilink / cite-link className 字面（給 test selector），同時也加上 Tailwind utility classes。CSS 本體用 codebus-app/src/index.css 寫成 `@layer components` 內的兩條 selector 而非全 utility-only —— 因為 WP11 CSS spec 明確列了字面 class name、保留可讀性。

**Tailwind v4 token 注意**：per Tailwind v4 token emission 教訓，若新增 color token（例如 border-strong 若尚未存在），必須同 change 確認有 consumer（utility 或 raw var()）才會 emit。Apply Task 1 含 grep 驗 border-strong 是否已存在的 token；若不存在 → 同 change 加 token + consumer。

**Alternative**：純 raw CSS class。Rejected：codebus 整體靠 Tailwind utility、純 raw CSS 兩套 styling source 會分裂。
**Alternative**：純 utility-only 不留 plain-wikilink className。Rejected：AUDIT WP11 CSS spec 明確點名兩個 class name、test 也用同名查；保留 class name 對 spec 可讀性 + test selector 友善。

### 旅行日誌 footer slot 放 WikiTree 不是 WikiPreview

WP-tree-footer 明確是「Wiki tree 底部」。實作：

- 在 WikiTree render 完所有 bucket 後加一個 div（data-testid `wiki-tree-footer-slot`），列「旅行日誌」row（lucide BookOpen icon + label）。
- 上方 18px gap + `border-t border-border`（一條 hairline）。
- 「旅行日誌」label 走 i18n key workspace.wiki.travelLogLabel。
- 點 row → 跟其他 wiki tree row 一樣走 onSelectSlug(slug) callback、slug 是 `log`（reuse 既有 log.md system page）。

**WK2 連動**：原 OTHER bucket 解散：Wiki Index 移到 tree 最上方當 vault 入口（slug 是 `index`）、log.md 從 OTHER bucket 拿掉、改放到 footer slot。Apply Task 含 grep 看 WikiTree.tsx OTHER bucket 怎麼判斷的（可能是 fallback 路徑、可能是 hard-code），對應改。

**Alternative**：放 WikiPreview 底部。Rejected：WP-tree-footer spec 明確位置是 tree、非 page reader。

### WK-EMPTY hero CTA 跑 setActiveTab('goals') + open NewGoalModal

WK-EMPTY-1 spec：CTA「→ 跑一個 goal 開始」auto setActiveTab('goals') + **強烈建議**再 open NewGoalModal。本 change 採用「兩步都做」：

- Click CTA → 經由 WikiTab 接 callback 上拋給 Workspace、Workspace setActiveTab('goals') + open NewGoalModal（不 prefill goal text、因為 user 還沒有要改的 page）。
- 既有 useWorkspaceStore / props chain：apply 階段 grep 看 active tab state 在哪裡、open NewGoalModal 既有 trigger 在哪、reuse 同一 trigger。

**Alternative**：只 setActiveTab、不 open modal。Rejected：WK-EMPTY-1 spec 寫「強烈建議再 open」、user 在 Wiki empty 沒接續行為的出口；強烈建議落實成「兩步都做」。

### WP-empty hint card 跟 WK-EMPTY hero 是兩個不同 component

- WP-empty-page（有 page 未選 page）= reader pane 顯示 36px 📂 hint card：純文字 hint、沒有 CTA、用 emoji 不是 lucide icon。
- WK-EMPTY-1（完全沒 page）= 整個 WikiTab area 顯示 56px lucide Folder hero icon + h-empty title + 副標 + amber CTA：完整 empty hero。

兩個視覺密度不同（hint card 比 empty hero 輕、避免在「只是還沒選 page」就大張旗鼓），分開：

- WikiTab 內判斷 hasPages false → render WikiEmptyHero（直接 inline 在 WikiTab body）。
- hasPages true 且 currentPath null → render WikiPreview 內的 unselected hint card（直接 inline 在 WikiPreview body=null branch）。
- 既有 WikiPreview body=null branch 目前 render 空 div、本 change 改成 render hint card。

## Implementation Contract

**Behavior**:

- 進 Wiki tab → 有 page → 選 page：WikiPreview 頂部出現 metadata bar（Last updated by goal · time-ago · N sources）；底部出現 edit hint footer（「想改這頁？跑一個 goal...」）。
- 進 Wiki tab → 有 page → 未選 page：WikiPreview 渲染區（之前空 div）出現 hint card 📂 + 「選一頁開始讀。」+ 「或點下方旅行日誌看 codebus 跑過什麼。」
- 進 Wiki tab → 完全沒 page：取代既有一行 hint，render 56px Folder hero icon + h-empty + 副標 + amber CTA button「→ 跑一個 goal 開始」。
- Wiki tree 底部出現「旅行日誌」row（hairline 分隔、fg-tertiary 色）；原 OTHER bucket 消失；Wiki Index 在 tree 最上方。
- Wikilink resolvable 從 inline 色 改吃 plain-wikilink className + Tailwind 對應 utility；unresolvable 保留現況樣式。
- Quiz button 從 generic secondary 改 amber primary；i18n key workspace.wiki.quizMeOnThis zh value 從「考我這頁」改「Quiz 這頁」。

**Interface / Data shape**:

- WikiPageMetadataBarProps：
  - goalLast: string or null（frontmatter.goals[last]；空陣列 → null）
  - updatedIso: string（frontmatter.updated）
  - wikilinkCount: number（transformBodyWikilinks(body).slugs.length）
  - onGoalClick: (goalId: string) => void
- Component 內：goalLast 為 null → 整段「Last updated by」省略；wikilinkCount 小於 1 → 整段「· N sources」省略；updatedIso 走 time-ago format helper（reuse common.minutesAgo / common.hoursAgo / common.daysAgo）。
- WikiPreviewProps 加新 prop onRequestNewGoal?: (prefilledText: string) => void，用來把 edit hint click 上拋。
- WikiTabProps 加 onWikiEmptyCta?: () => void，WK-EMPTY CTA click 上拋。
- 上述兩 callback 在 Workspace 層接、配合既有 setActiveTab / openNewGoalModal seam。
- i18n 新 key（key 名 final，apply 不再 rename）：
  - workspace.wiki.metadata.lastUpdatedBy ── 「Last updated by」/「最後更新者」
  - workspace.wiki.metadata.sourcesSuffix ── 「sources」/「處引用」
  - workspace.wiki.editHint.text ── 「Want to edit this page? Run a goal and tell codebus what to change →」/「想改這頁？跑一個 goal 跟 codebus 說該怎麼改 →」
  - workspace.wiki.editHint.linkLabel ── 「Run a goal」/「跑一個 goal」（給 link text 精確切片）
  - workspace.wiki.unselectedHint.title ── 「Pick a page to start reading.」/「選一頁開始讀。」
  - workspace.wiki.unselectedHint.subtitle ── 「Or click the travel log below to see what codebus has been up to.」/「或點下方旅行日誌看 codebus 跑過什麼。」
  - workspace.wiki.emptyHero.title ── 「No wiki pages yet」/「還沒有任何 wiki page」
  - workspace.wiki.emptyHero.subtitle ── 「Run a goal — codebus will read along and turn your mental model into postcards here.」/「跑一個 goal，codebus 就會邊讀邊把 mental model 整理成這裡的明信片」
  - workspace.wiki.emptyHero.cta ── 「→ Run a goal to start」/「→ 跑一個 goal 開始」
  - workspace.wiki.travelLogLabel ── 「Travel log」/「旅行日誌」
- 修改既有 key value（en 不變、zh 改）：
  - workspace.wiki.quizMeOnThis zh value：「考我這頁」→「Quiz 這頁」（Quiz 保留 jargon）

**Failure modes**:

- frontmatter.goals 空陣列 → metadata bar「Last updated by」段省略，其他兩段仍顯示。
- frontmatter.updated parse 失敗（不是 ISO date）→ time-ago helper 回 null、time-ago 段省略；不丟 exception。
- transformBodyWikilinks slugs 空 → N sources 段省略。
- 三段全省略 → metadata bar component 不 render（return null）。
- Edit hint click 時 currentPath 為 null → callback 不送（按理 hint 在 page 顯示時才出現、不會無 currentPath）。
- WK-EMPTY CTA click 時 onWikiEmptyCta 未注入 → no-op（不爆）。

**Acceptance criteria**:

- Unit test：
  - WikiPageMetadataBar.test.tsx ── render 全三段 / goalLast 為 null 省第一段 / wikilinkCount 為 0 省第三段 / updatedIso invalid 省第二段 / 三段全省 return null / goal click trigger callback。
  - WikiPreview.test.tsx ── edit hint footer 顯示 / link click 觸發 onRequestNewGoal 帶正確 prefill（修改 wiki/<rel-path> — ）/ resolvable wikilink 帶 plain-wikilink className / Quiz button 加 amber variant data attr / WP-empty hint card 在 body=null 時顯示。
  - WikiTab.test.tsx ── WK-EMPTY hero hero icon + 文案 + CTA / CTA click 觸發 onWikiEmptyCta。
  - WikiTree.test.tsx ── footer slot「旅行日誌」row 顯示 + click 觸發 onSelectSlug('log') / Wiki Index 在 tree 最上方 / OTHER bucket 不再 render。
- `pnpm tsc` 綠。
- `pnpm test` 綠。
- 真實 CDP smoke（zh + en、按 cdp-smoke 五雷流程）：開 codebus dogfood vault → Wiki tab → 選實際 wiki page → metadata bar 三段顯示 → 點 goal name 跳該 goal detail → 點 edit hint link 開 NewGoalModal 看 prefill → 點 wikilink 看樣式 + navigate → 故意 navigate 到不存在 slug 看 unresolvable → tree 底部看「旅行日誌」row → 完全空 vault 看 WK-EMPTY hero + CTA → 切 locale zh/en 兩次都驗。截圖存 codebus-app/scripts/.wiki-reader-smoke/。

**Scope boundaries**:

- 在 scope：上述 7 個 AUDIT 條目（WP2 / WP5 / WP10 / WP11 / WP-tree-footer / WP-empty-page / WK-EMPTY-1/2/3）、純 frontend、不動 Rust、不動 wikilink IPC、不動 Milkdown 引擎、不動 wiki file tree bucket 結構（除了 OTHER 解散 + footer slot）。
- 不在 scope：runs_referenced backend（需新 IPC）、wikilink visited state（沒 spec）、body-only frontmatter empty 渲染（沒 spec）、metadata bar 擴大欄位（spec lock 禁）、wiki tree 重構（範圍超出本 change）、跨 surface 變動（6.1 / 6.2 / 5.x）。

## Risks / Trade-offs

- **Tailwind v4 token emission**：border-strong token 若不存在，新加要同 change 塞 consumer。→ Mitigation：Apply Task 1 grep border-strong 確認 token 狀態、缺則加 token + consumer 同 commit。
- **WikiTree OTHER bucket 解散影響面**：grep 不確定 OTHER bucket 是 fallback path 還是 hard-coded list。→ Mitigation：Apply Task 含 grep OTHER 校準現況再改、保留 fallback 行為（unknown folder 走原 OTHER 路徑、但不 render bucket header）。
- **Edit hint prefill 取 page rel path**：useWikiStore.pages[slug] 是否帶 path 欄位不確定。→ Mitigation：Apply Task 含 grep WikiPageMeta shape 確認；若無 path 欄位、退而求其次 currentPath slug + frontmatter type folder 合成。
- **CDP smoke 五雷**：WebView2 / React batching / Tailwind transition / CDP eval 副作用 / Settings modal locale 切。→ Mitigation：CDP smoke 走前掃 cdp-smoke 五雷、settings 切 locale 必須用 settings-save testid。
- **i18n key naming convention 例外**：codebus 整體 snake_case、i18n key 既有 camelCase。→ Mitigation：本 change 沿用既有 camelCase；apply 階段 grep 確認 key naming 一致性、不引入 snake_case 新 key。
- **AUDIT 條目跨 7 條 + 範圍跨三個 component**：scope 偏大。→ Mitigation：tasks.md 按 AUDIT 條目拆分、單條目單 task chunk、可獨立驗收；工時上限 1-1.5 天、超過 stop 對齊。
- **5-bucket identifier disambiguation**：本 change 不展示 type identifier（spec lock metadata bar 沒這欄）、避開歧義；但 Quiz wizard / Wiki tree section label 有用到 —— apply 階段勿混淆 PageType enum singular 跟 folder 名 plural。
- **同名詞 disambiguation**：「Wiki tab」「Wiki file tree」「Wiki page reader」「Frontmatter metadata」「Metadata bar」「旅行日誌 footer slot」「WP-empty-page」七個詞已在上方 Pre-apply 校準段列出，apply 階段引用 AUDIT 條目 id 而非泛指「metadata」/「footer」避免歧義。
