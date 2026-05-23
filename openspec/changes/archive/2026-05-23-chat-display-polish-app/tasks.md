<!--
Each task description states the behavior delivered AND the verification target.
File paths appear only as supporting locator context, never as the task itself.
-->

## 1. AssistantMarkdownBlock 改造（Chat Assistant Message Markdown Rendering and Wiki Citation Links）

- [x] 1.1 RED：為 `Chat Assistant Message Markdown Rendering and Wiki Citation Links` 要求的新增場景在 `codebus-app/src/components/workspace/ChatTranscript.test.tsx` 新增 3 條測試——(a) GFM 表格 markdown input → DOM 有 `<table>` 與至少一個 `<th>` 含 header 文字；(b) 純文字 `[[modules/auth]]` + `useWikiStore.pages["modules/auth"]` 存在 → 渲染為 resolvable button、click 觸發 `onWikiLinkClick("modules/auth")`、widget 轉 collapsed；(c) `[[nonexistent]]` + pages 無此 slug → 渲染為 dim `<span>` 帶 `title="Page not found"`、click 為 no-op、`onWikiLinkClick` 不被呼叫。完成行為：3 條新測試 FAIL（GFM plugin 未掛、`[[slug]]` 未預處理、urlTransform 未放行 codebus://）。驗證方式：`npm test -- ChatTranscript` 顯示 3 條新增名稱為 failed AND 既有 3 條（chat-wiki-link / chat-external-link / chat-inert-link）仍 pass（暫時、會在 1.2 中破）。
- [x] 1.2 RED：更新既有 3 條 chat-link 測試 assertion 對齊 slug-based callback contract——把 `expect(onWikiLinkClick).toHaveBeenCalledWith("wiki/modules/auth.md")` 改成 `expect(onWikiLinkClick).toHaveBeenCalledWith("modules/auth")`（從 regex capture group 抽出的 slug）；inert / external 測試的 assertion `onWikiLinkClick` not called 保持不變。同時更新測試 fixture 把該 vault path 的 `pages` map 預先注入 `useWikiStore` 讓 resolvable 判定為 true。完成行為：3 條既有測試 assertion 對齊新 contract、目前 FAIL（code 未改）。驗證方式：`npm test -- ChatTranscript` 顯示 wiki-link / 相關 fixture-driven 測試 failed（slug 抽取邏輯未實作）。
- [x] 1.3 GREEN：在 `AssistantMarkdownBlock` 實作所有 spec 要求——(a) `useWikiStore((s) => s.pages)` 訂閱 pages map；(b) 用 `transformBodyWikilinks(text)` 預處理；(c) `<ReactMarkdown remarkPlugins={[remarkGfm]} urlTransform={(url) => url}>`；(d) `components.a` 改為四 branch（codebus://wiki/ → 抽 slug + 查 pages → resolvable/unresolvable；`^wiki/(.+)\.md$` → capture group 抽 slug + 查 pages → 同 resolvable/unresolvable；`^https?:` → 既有 external opener；其他 → inert span）；(e) resolvable button click 傳 **slug**（非 href）給 `onWikiLinkClick`。完成行為：1.1 + 1.2 所有測試 pass，既有 external / inert / 其他 chat 測試仍 pass。驗證方式：`npm test -- ChatTranscript` 全綠。
- [x] 1.4 GREEN：在 `AssistantMarkdownBlock` 加 `components.table` override 套 Tailwind 樣式（最小：`border-collapse border border-border text-xs`），以及 `components.th` / `components.td` 各加 padding + border。完成行為：spec scenario "GFM table renders as table element" 中 `<table>` 元素有可讀視覺呈現（不裸 unstyled）。驗證方式：1.1 GFM table 測試已驗 DOM 存在；視覺呈現的合理性透過 task 3.2 的 CDP manual sanity 確認。
- [x] 1.5 contract 修正：把 `ChatTranscriptProps.onWikiLinkClick` 與 `TurnBlockProps.onWikiLinkClick` 與 `AssistantBlockProps.onWikiLinkClick` 的型別從 `(href: string) => void` 改成 `(slug: string) => void`，更新對應 JSDoc 註解（spec line 44-47 的描述也對應修正不變動，但 prop type 統一）。完成行為：TypeScript 編譯通過、type 與 Workspace.onSelectPage(slug) 對齊。驗證方式：`npm run typecheck` 或 `npm test`（Vitest 跑前先過 type）綠。

## 2. ChatTranscript Props 鏈傳遞核對

- [x] 2.1 確認 `Workspace.tsx::onSelectPage(slug)` 與本 change 後的 `ChatTranscript.onWikiLinkClick(slug)` contract 端到端一致——`Workspace.tsx:212-218` 的 `onSelectPage` 既有 signature `(slug: string) => void` 與 `loadPage(vault.path, slug)` 流程不需動。手動 grep 確認沒有其他 ChatTranscript 消費者傳 href-based callback。完成行為：上下游 contract 對齊、無遺漏的 caller。驗證方式：`grep -rn "onWikiLinkClick" codebus-app/src` 結果 review。

## 3. 整合與最終驗證

- [x] 3.1 全 app 測試：`npm test` 在 `codebus-app/` 全綠，含本次新增 3 條 + 對齊既有 3 條 chat 測試 + 既有所有測試。完成行為：app 端 vitest 0 failures。驗證方式：CLI 輸出。
- [x] 3.2 手動 sanity（CDP）：啟 `cargo tauri dev` 含 `--remote-debugging-port=9222`、開啟既有 vault、打開 chat widget、貼入測試用 markdown（含 GFM 表格 + `[[some-existing-slug]]` + `[[nonexistent]]` + 外部 https）作為一個 assistant turn（透過 useChatStore.setState 注入或 mock turn）、觀察渲染：(a) 表格實際 render 成 `<table>`；(b) resolvable wikilink click 切到 wiki tab + widget 摺疊；(c) unresolvable wikilink 灰顯 + 不可點；(d) https 連結走 Tauri opener。完成行為：四個 case 在真實 binary 下可觀察。驗證方式：CDP `text` dump + screenshot 至少一張到 PR 描述。
- [x] 3.3 spectra validate：`spectra validate chat-display-polish-app` 通過——spec/tasks 一致性、無 forbidden words、Scenario 格式皆正確。完成行為：validate 0 errors 0 warnings。驗證方式：CLI 輸出。
- [x] 3.4 cross-platform reasoning：純 TSX / React 改動，無 OS-specific syscall。Windows local 通過後 Mac/Linux 邏輯自動一致（依賴 react-markdown / remark-gfm 跨平台行為）。PR 描述記錄此 reasoning。完成行為：跨 OS 推理留底。驗證方式：PR description 含此段落。
