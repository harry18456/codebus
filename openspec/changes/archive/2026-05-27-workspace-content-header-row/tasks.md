<!--
Tasks reference:
- design.md headings: Pre-apply 校準, 抽共用 component TabContentHeader,
  Goals empty state 三段式 layout, Goals populated RECENT SectionLabel,
  Quiz tab 套 TabContentHeader, i18n key 處理, 共用 TabContentHeader 進 design-system spec
- spec requirements: Goals Overview List and Filter (modified),
  Quiz Tab Content Header Row (added), TabContentHeader component (added)

Parallel markers ([P]) reflect file-level independence given parallel_tasks=true in .spectra.yaml.
-->

## 1. Pre-apply 校準 + tdd 起步

- [x] 1.1 跑 ground truth grep 校準 design.md「Pre-apply 校準（apply 第一步 grep 後對應更新）」段四項假設（R4 examples wiring 未完成、QuizTab 已有 h2/heading key、QuizTab 已有 `+ New quiz` button、SectionLabel caps 存在）。任何新差異補進 design.md 同段；新增的 site 若改變 scope → stop 找 user。**Verification**：把 grep 命令與結果（site count、檔案行範圍）回貼 design.md「Pre-apply 校準（apply 第一步 grep 後對應更新）」段 + chat 報告差異列表。
- [x] 1.2 先掃一次 `project_cdp_smoke_webview2_pitfalls` 5 雷 checklist（prefers-reduced-motion CSSOM rule / React batching 拆 eval / focus sleep ≥500ms / cdp click retry 副作用 / settings-save testid 切 locale）並寫成本次 CDP smoke run book 留在 `codebus-app/scripts/.content-header-smoke/RUNBOOK.md`。**Verification**：runbook 檔案存在且 5 雷各對應一條 mitigation。

## 2. 新增 TabContentHeader component（tdd-first）

- [x] 2.1 [P] 寫 `codebus-app/src/components/ui/TabContentHeader.test.tsx`：覆蓋 spec requirement「TabContentHeader component」全部 6 個 scenario（title-only / + subtitle / cta 無 chip / cta + chip / 無 cta 抑制 chip / testId 落到 root）。先紅。**Verification**：`pnpm test TabContentHeader` 顯示 6 個 test 全紅（component 還沒實作）。
- [x] 2.2 實作 `codebus-app/src/components/ui/TabContentHeader.tsx`，props shape 對齊 design.md「抽共用 component TabContentHeader」段；root row 套 `data-tauri-drag-region` + `pr-[160px]` + `text-h-row` h1 + meta 副標 + 右側 cta+chip group。**Verification**：2.1 的 6 個 test 全綠 + `pnpm tsc` 綠。
- [x] 2.3 [P] shortcut chip 樣式對齊 sidebar 既有 `⌘K` chip：grep `design-handoff/design_files/components/sidebar.jsx` 與 `codebus-app/src/components/layout/` 找既有 chip className/token、`<TabContentHeader>` 借用相同 class。**Verification**：chip 視覺與 sidebar `⌘K` chip 在 CDP screenshot 對照下無 token 偏差（chat-report 並截圖到 `.content-header-smoke/chip-compare.png`）。

## 3. 新增 i18n key（i18n key 處理）

- [x] 3.1 在 `codebus-app/src/i18n/messages.ts` 新增 6 個 key 雙 locale value（`workspace.goals.headerTitle` / `workspace.goals.headerSubtitle` / `workspace.goals.emptyHeroTitle` / `workspace.goals.emptyHeroSubtitle` / `workspace.quiz.headerTitle` / `workspace.quiz.headerSubtitle`），文案對齊 design.md 候選表；apply 前先 grep `emptyHint` 確認是否可重用、避免新增冗餘 key。**Verification**：`pnpm test workspace quiz` i18n test 綠且 messages.ts 兩 locale 同 key set 完整（`workspace.test.ts` / `quiz.test.ts` 跑通）。
- [x] 3.2 [P] 在 `codebus-app/src/i18n/workspace.test.ts` 跟 `quiz.test.ts` 加 assertion：新 6 key 在 zh + en 兩 locale 都 defined、value 非空、不等於 key 字面（避免 fallback 假綠）。**Verification**：相應 unit test 綠。

## 4. Goals tab 改套 TabContentHeader + 三段式 empty + RECENT label（Goals Overview List and Filter）

- [x] 4.1 更新 `codebus-app/src/components/workspace/GoalsTab.test.tsx`：覆蓋 spec requirement「Goals Overview List and Filter」新加的 5 個 scenario（content header row、RECENT section label、empty 三段式、pre-fill 走 i18n、locale 切換時無英文殘留）。先紅。**Verification**：`pnpm test GoalsTab` 6+ 個 assertion 紅。
- [x] 4.2 改 `GoalsTab.tsx`：把現有 `flex justify-end border-b border-border p-3 pr-[160px]` topbar 換成 `<TabContentHeader title={t("workspace.goals.headerTitle")} subtitle={t("workspace.goals.headerSubtitle")} cta={<Button …>+ New goal</Button>} shortcutChipText="N" testId="tab-content-header-goals" />`；populated 分支在 `<ul>` 前插入 `<SectionLabel variant="caps">RECENT</SectionLabel>`；empty 分支改三段式（hero region 用 `workspace.goals.emptyHeroTitle/Subtitle`、examples 改用 `t("workspace.goals.examplePlaceholder1..3")` 取代英文常數）。**Verification**：4.1 全綠 + `pnpm tsc` 綠 + run-dev 開 vault 看 Goals tab empty + populated 兩態截圖存 `.content-header-smoke/goals-empty-zh.png` / `goals-populated-zh.png`。
- [x] 4.3 [P] 把 `GOAL_EXAMPLES` 常數移除（既然走 i18n key、不再需要 source-level 常數）；保留 NewGoalModal prefill 流程不動（pill click → `openModalWith(t("...examplePlaceholder1"))`）。**Verification**：grep `GOAL_EXAMPLES` 全 repo 無命中、`GoalsTab.test.tsx` pre-fill click test 仍綠。

## 5. Quiz tab 套 TabContentHeader（Quiz Tab Content Header Row）

- [x] 5.1 更新 `codebus-app/src/components/workspace/QuizTab.test.tsx`：覆蓋 spec requirement「Quiz Tab Content Header Row」三個 scenario（history view 渲染 header / 非 history phase 不渲染 / CTA 點擊 transition）。先紅。**Verification**：`pnpm test QuizTab` 三個 assertion 紅。
- [x] 5.2 改 `QuizTab.tsx`：把現有 `<div data-tauri-drag-region className="flex items-center justify-between border-b border-border p-3 pr-[160px]">` 含 `<h2>workspace.quiz.tab.heading</h2>` 跟 `+ New quiz` 的 wrapper，在 `phase === "history"` 時改用 `<TabContentHeader title={t("workspace.quiz.headerTitle")} subtitle={t("workspace.quiz.headerSubtitle")} cta={<Button data-testid="new-quiz">+ New quiz</Button>} testId="tab-content-header-quiz" />`；其他 phase 維持現況不動。**Verification**：5.1 全綠 + `pnpm tsc` 綠 + CDP smoke 切到 quiz tab 看 empty + populated history 兩態截圖存 `.content-header-smoke/quiz-empty-zh.png` / `quiz-populated-zh.png`。

## 6. 真實 CDP smoke 驗證（Implementation Contract Acceptance criteria）

- [x] 6.1 跑真實 CDP smoke（per Implementation Contract acceptance criteria 第 3 點）：開 codebus-app `--remote-debugging-port=9222` + `codebus-app/scripts/cdp.mjs`；建一個空 vault 跟一個有 1-3 個 goal 的 vault；驗 Goals empty、Goals populated、Quiz empty history、Quiz populated history 四個畫面，zh + en locale 切換用 `settings-save` testid。**Verification**：所有截圖（8 張）存 `.content-header-smoke/`、chat 報告每張對應的 spec scenario 通過 / 失敗清單。
- [x] 6.2 按 N 鍵驗 shortcut 觸發 New Goal modal（既有行為保留、不因 `<TabContentHeader>` 重排而失效）。**Verification**：CDP eval `dispatchEvent KeyboardEvent` 後抓到 `new-goal-modal` testid 開啟、截圖存 `.content-header-smoke/shortcut-n-trigger.png`。
- [x] 6.3 4B amber bar ↔ 4C h1 視覺呼應確認：切換 nav 至 Goals / Quiz 兩 tab、CDP screenshot 同時抓左側 nav active row 跟右側 content h1，視覺對比 amber bar 顏色與 h1 token。**Verification**：對比結果寫成 `.content-header-smoke/4b-4c-visual-handshake.md`，列出比較結論（pass / mismatch）。
- [x] 6.4 跨 tab 一致性確認：對比 Goals tab 跟 Quiz tab content header 在 DOM 結構（`data-testid` 抓出來的元素 tree）跟 visual layout 上是否完全一致（除 subtitle / chip optional 差異）。**Verification**：DOM tree diff + visual diff 寫成 `.content-header-smoke/cross-tab-consistency.md`。

## 7. 收尾

- [x] 7.1 [P] `pnpm tsc` 跟 `pnpm test` 全 repo 跑一次完整通過。**Verification**：兩個指令 exit 0 + chat 貼 last lines 確認。
- [x] 7.2 archive 階段在 `codebus-app/design-handoff/AUDIT.md` R2/R3/R4/R5/R6 + GP1/GP2/GP3 + QE4/QL6 標 `archived 2026-05-27`（注意：今天就是 2026-05-27）。**Verification**：grep `archived 2026-05-27` 在 AUDIT.md 對應 9 個 section 都命中。
- [x] 7.3 archive 階段確認 Phase 4（4A + 4B + 4C）layout structure 階段完整收尾的 doc note，準備進 Phase 5；不另寫文件（per memory「不要在長 apply session 亂 checkpoint」），把段落補在 AUDIT.md Phase 4C 落地確認段即可。**Verification**：AUDIT.md `### Phase 4C` 段下方新增「✅ archived 2026-05-27」與 Phase 4 整體收尾 confirm 句。
