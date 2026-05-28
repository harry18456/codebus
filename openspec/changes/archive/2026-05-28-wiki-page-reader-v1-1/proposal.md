## Why

Phase 6 v1.1 mock landing 第三塊。Wiki page reader 在 design v1.1 已 spec lock 多個 gap（WP2 / WP5 / WP10 / WP-tree-footer / WP-empty-page / WP11 / WK-EMPTY-1/2/3），現況實作只覆蓋基本 markdown render + wikilink resolvable/unresolvable 兩 state，缺 metadata bar、缺 "跑 goal 改" edit hint、wiki tree 缺旅行日誌 footer slot、未選 page 缺 hint card、完全沒 page 的 empty state 視覺超陽春。本 change 把這些 design v1.1 spec lock 條目一次性落地、不擴新範圍。

## What Changes

- **WP2 · Page metadata bar**（page reader 頂部、title 下方）：新增單行三段 bar `Last updated by <goal>` · `<time-ago>` · `<N> sources`。`<goal>` 取 `frontmatter.goals[last]` 可點跳該 goal detail（reuse 既有 router seam）；`<time-ago>` 由 `frontmatter.updated` 推導、reuse `common.minutesAgo` / `common.hoursAgo` / `common.daysAgo`；`<N> sources` 是 body 內 wikilink count（不是 frontmatter `sources[]`）、< 1 時整段不顯示。禁止加 tags / word count / view count / authors。
- **WP5 · Edit hint footer**（page reader 底部、上方 24px gap）：「想改這頁？跑一個 goal 跟 codebus 說該怎麼改 →」`fg-tertiary` 12.5px、「跑一個 goal」是 link、點 → 開 NewGoalModal 並 prefill `goal: "修改 wiki/<page-path>.md — "` 前綴。reuse 既有 NewGoalModal IPC seam。
- **WP10 · 底部 action button polish + i18n**：`workspace.wiki.quizMeOnThis` 翻譯改成「Quiz 這頁」（Quiz 保留 jargon）；`Quiz me on this` button 改 amber 主色強調可測驗、`Open in Obsidian` 保留 secondary。
- **WP11 · Wikilink 雙樣式 lock**：定義 `.plain-wikilink`（body navigation、underline `border-strong` + hover `accent`）與 `.cite-link`（citation block、mono dashed-amber-underline）兩 variant CSS；既有 `WikiPreview.tsx` resolvable inline style 改吃 `.plain-wikilink`、unresolvable 樣式保留。不引入 visited state。
- **WP-tree-footer · Wiki tree 旅行日誌 footer slot**：`WikiTree.tsx` 底部加獨立區（不在任何 bucket 裡）、列「旅行日誌」當 system page entry、上方 18px gap + 一條 hairline 分隔、`fg-tertiary` 色。連動 WK2：原 OTHER bucket 解散、Wiki Index 移到 tree 最上方當 vault 入口、旅行日誌移到底部當 system slot。
- **WP-empty-page · 有 page 但未選 page 的 hint card**：reader pane 顯示 36px 📂 emoji + 16px「選一頁開始讀。」+ 12px「或點下方旅行日誌看 codebus 跑過什麼。」hint card；layout = tree 左有內容、reader pane 顯示 hint card。
- **WK-EMPTY-1/2/3 · 完全沒 page 的 empty state**：`WikiTab.tsx:52-66` 純一行 hint 升級成 hero layout — 56px lucide `Folder` icon + h-empty「還沒有任何 wiki page」+ 副標「跑一個 goal，codebus 就會邊讀邊把 mental model 整理成這裡的明信片」+ amber primary CTA 「→ 跑一個 goal 開始」（auto `setActiveTab('goals')` + 強烈建議再 open NewGoalModal）。en 候選文案「No wiki pages yet — run a goal and codebus will start writing」。
- **i18n key 新增**（既有 prefix `workspace.wiki.*` + camelCase 命名 convention）：metadata bar 三段 label / time-ago format / edit hint 文案 / WP-empty hint card 兩行 / WK-EMPTY hero + 副標 + CTA / WP10 quiz 翻譯更新。新 key 估算 8-12 條、en + zh 兩 bundle 都加。

## Non-Goals

留在 design.md「Non-Goals」段詳列。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-workspace`: 既有 Wiki Tab / Wiki Page Preview / Wikilink Resolution 三組 requirement 各自加 metadata bar / edit hint footer / quiz button amber tint / tree footer slot / WP-empty hint card / WK-empty hero CTA / .plain-wikilink + .cite-link 雙 variant 樣式約束。所有變動都是 UI 層 additive、不動現有 IPC contract。

## Impact

- Affected specs:
  - openspec/specs/app-workspace/spec.md（modified · 加 metadata bar / edit hint / tree footer / WP-empty / WK-empty / wikilink 雙樣式 requirement）
- Affected code（純 frontend、不動 Rust）:
  - Modified:
    - codebus-app/src/components/workspace/WikiPreview.tsx（加 metadata bar / edit hint footer / wikilink className 切換 / quiz button amber）
    - codebus-app/src/components/workspace/WikiTab.tsx（WK-EMPTY hero CTA + auto setActiveTab）
    - codebus-app/src/components/workspace/WikiTree.tsx（旅行日誌 footer slot + Wiki Index 移到頂、解散 OTHER bucket）
    - codebus-app/src/lib/milkdown-wikilink.tsx（resolvable 改吃 .plain-wikilink className、移除 inline color style）
    - codebus-app/src/i18n/messages.ts（新增 workspace.wiki.* key 8-12 條 / zh + en 兩 bundle）
    - codebus-app/src/index.css（或 design-system token 檔、依 grep 結果）— `.plain-wikilink` + `.cite-link` 雙 variant CSS 落地
    - codebus-app/src/components/workspace/WikiPreview.test.tsx（補 metadata bar / edit hint / WP-empty test）
    - codebus-app/src/components/workspace/WikiTab.test.tsx（補 WK-EMPTY hero + CTA test）
    - codebus-app/src/components/workspace/WikiTree.test.tsx（補 footer slot test）
    - codebus-app/design-handoff/AUDIT.md（archive 階段 mark WP2 / WP5 / WP10 / WP11 / WP-tree-footer / WP-empty-page / WK-EMPTY-1/2/3 為 archived 2026-05-28）
  - New:
    - codebus-app/src/components/workspace/WikiPageMetadataBar.tsx（單行三段 bar component、抽出供 test 直接 mount）
  - Removed:
    - (none)
