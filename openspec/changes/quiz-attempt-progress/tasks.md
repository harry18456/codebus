<!--
Traceability：每個 task 標明交付行為、驗證目標、對應 spec requirement 與 design 決策。TDD：標 (RED) 先寫失敗測試，(GREEN) 才實作到綠。audit：path 守衛/容錯走 Scoundrel+Confused 雙視角。
-->

## 1. 進度 sidecar 核心單元（design D1 / D2 / Implementation Contract「Sidecar schema & tolerance」「Atomic write」）

- [x] 1.1 (RED) 在 `codebus-core/src/verb/quiz_progress.rs` 的測試模組新增單元測試：`QuizProgress { schema_version, answers:[{q,selected,correct}], status, started_at, completed_at }` 之 (a) 缺檔 → 回 not-started（非 error）；(b) 壞檔/亂碼 → not-started（不 panic）；(c) round-trip 序列化/反序列化一致；(d) 未知 JSON key 忽略、`schema_version` 較新仍讀已知欄位；(e) atomic write 覆寫既有 sidecar 後內容為第二次、且同目錄無殘留 `.tmp`。對應 design D1/D2 與 spec `quiz` / Quiz Storage Layout and Retry Semantics。驗證：測試先 fail（模組/型別未存在，編譯紅）。
- [x] 1.2 (GREEN) 實作 `codebus-core/src/verb/quiz_progress.rs`：`QuizProgress` 型別 + serde；`read_progress(path) -> QuizProgress`（缺檔=not-started、壞檔容錯=not-started、unknown key 忽略，鏡像 codebus config tolerance）；`write_progress(path,&QuizProgress)`（同目錄 temp + `fs::rename` 覆寫，Windows 亦覆寫）；於 `codebus-core/src/verb/mod.rs` 導出。對應 design D2。驗證：1.1 全綠；`cargo test -p codebus-core` 不回歸。依賴 1.1。

## 2. Tauri IPC：read/write_quiz_progress + registry 23→25（design D2 / Implementation Contract「Containment」「Registry」）

- [x] 2.1 (RED) 在 `codebus-app/src-tauri` 新增測試：(a) `ipc/mod.rs` 的 count 測試期望 `REGISTERED_COMMANDS.len()==25` 且名單含 `read_quiz_progress`、`write_quiz_progress`；`tests/keyring_ipc.rs` count 23→25；(b) `read_quiz_progress(vault_path,path)` 對缺 sidecar 回 not-started、對樹外 path 以 `AppError::Invalid{field:"path"}` 拒絕；(c) `write_quiz_progress` 後 `read_quiz_progress` round-trip 一致、樹外 path 拒絕。對應 design D2 與 spec `app-workspace` / Tauri IPC Commands for Quiz Plan and Generate Lifecycle。驗證：測試先 fail（命令未存在、count 仍 23）。
- [x] 2.2 (GREEN) 在 `codebus-app/src-tauri/src/ipc/quiz.rs` 實作 `read_quiz_progress` / `write_quiz_progress`，path 須 resolve 在 vault `.codebus/` 樹下否則 `AppError::Invalid{field:"path"}`（containment 對齊既有 `read_quiz_attempt`，audit Scoundrel：禁無界讀寫），委派 1.2 core 單元；於 `ipc/mod.rs` 的 `generate_ipc_handler!` 與 `REGISTERED_COMMANDS` 註冊；count 測試與 `tests/keyring_ipc.rs` 改 25；於 `codebus-app/src/lib/ipc.ts` 加 typed `readQuizProgress` / `writeQuizProgress` wrapper（含 `QuizProgress` TS 型別）。對應 design D2。驗證：2.1 全綠；既有 tauri 測試不回歸。依賴 2.1。

## 3. 答題持久化 + 續答（design D3 / Implementation Contract「Resume」）

- [x] 3.1 (RED) 在 `codebus-app/src/components/workspace/QuizAnswering.test.tsx` 與 `QuizTab.test.tsx` 新增測試：(a) 每次送出 → 呼叫 `writeQuizProgress`，payload 含該題 `selected`/`correct`、`status:"in_progress"`；末題送出 → `status:"completed"` + `completed_at`，且全程不呼叫 `spawn_quiz_*`；(b) 開啟一個 sidecar 已答 Q1、Q2（共5題、in_progress）的 attempt → 答題視圖從 Q3 起（第一個不在 answers 的題號）。對應 design D3 與 spec `app-workspace` / Quiz Answering and Summary。驗證：測試先 fail（現 QuizAnswering 無持久化、無 resume）。依賴 2.2。
- [x] 3.2 (GREEN) `QuizAnswering.tsx`：每次 submit 計算更新後 answers 呼叫 `writeQuizProgress`（status 規則同上）；接受一個初始 progress（由 QuizTab 載入 `readQuizProgress` 傳入）並從第一個未答題號起始；不 spawn。對應 design D3。驗證：3.1 轉綠；`npx vitest run` 全綠不回歸；`npm run typecheck` 乾淨。依賴 3.1。

## 4. History 狀態徽章/路由 + Review 取代 raw md（design D4 / D5 / Implementation Contract「Derived status」「Review replaces raw md」「Redo this」）

- [x] 4.1 (RED) 在 `codebus-app/src/components/workspace/QuizTab.test.tsx` 與新增 `QuizReview.test.tsx` 寫測試：(a) 每列依 `readQuizProgress` 衍生徽章：無 sidecar `0/N`、in_progress `X/N`、completed `X/N · score% · pass|fail`（pass 用 settings `app.quiz.pass_threshold`）；(b) 點列路由：not-started/in-progress→`QuizAnswering`、completed→`QuizReview`；(c) `QuizReview` 逐題顯示「使用者選擇 vs 正解 + 解釋」，**非** raw `<pre>` md（斷言無原 raw-md testid）；(d) `QuizReview` 的 `[重做此份]`（testid `quiz-redo-this`）→ 重置該 attempt sidecar 並回 `QuizAnswering` 第1題、**不**呼叫 `spawn_quiz_*`；(e) 既有「看過程」中央 modal 仍存在於 Review（`events_log` 非 null 時）。對應 design D4/D5 與 spec `app-workspace` / Quiz History List。驗證：測試先 fail（現 raw md、無徽章/路由/Review/redo）。依賴 2.2。
- [x] 4.2 (GREEN) 新增 `codebus-app/src/components/workspace/QuizReview.tsx`（吃 attempt md + progress，逐題渲染 user-choice vs Answer + Explanation，含 `[重做此份]` 與既有「看過程」入口 reuse `QuizGenerationLog`+Dialog）；`QuizTab.tsx`：history 每列 `readQuizProgress` 算徽章；`openAttempt` 依衍生 status 路由（completed→`QuizReview` 取代現行 `<pre>{attemptMd}</pre>`；其餘→`QuizAnswering` 帶初始 progress）；`重做此份` 寫 not-started sidecar 後進 answering 第1題。對應 design D4/D5。驗證：4.1 轉綠；`npx vitest run` 全綠不回歸；`npm run typecheck` 乾淨。依賴 4.1、3.2。

## 5. 全域回歸 + 驗收收尾（design Non-Goals 範圍邊界）

- [x] 5.1 全域回歸 sweep 並登記驗收：`cargo test -p codebus-core -p codebus-cli`、`cargo test --manifest-path codebus-app/src-tauri/Cargo.toml`、`cd codebus-app && npx vitest run && npm run typecheck` 全綠（彙總 0 failed）；progress sidecar 行為以上述 core/tauri/vitest 自動測試覆蓋（CLI 無答題 UI，依 Non-Goals 不另做 CLI 答題）；於 `docs/v3-app-roadmap.md` Deferred acceptance registry 新增 `quiz-attempt-progress` 條目，誠實登記（自動測試範圍 / GUI 互動驗收狀態 / macOS-Linux 仍 deferred to polish-ship）。對應 design Non-Goals。驗證：四套件彙總 0 failed；roadmap registry 條目存在且與實況一致（review 確認）。依賴 4.2。

## 6. Explanation 引用渲染成可導航 wikilink，移除 back-to-wiki 按鈕（design D6 / Implementation Contract「Explanation wikilinks」；spec `app-workspace` / Quiz Answering and Summary 改版）

- [x] 6.1 (RED) 在 `codebus-app/src/lib/quiz-parse.test.ts` 加測試：某題 explanation 為 `... [[auth-middleware-verification]] ... [[login-token-minting]] ... [[auth-middleware-verification]]` → `parseQuiz` 該題 `sources` === `["auth-middleware-verification","login-token-minting"]`（順序、去重）；無 `[[ ]]` → `sources` === `[]`。對應 design D6。驗證：測試先 fail（`parseQuiz` 尚無 `sources` 欄位）。
- [x] 6.2 (GREEN) `codebus-app/src/lib/quiz-parse.ts`：`QuizQuestion` 加 `sources: string[]`，`parseQuiz` 從該題 explanation 依序抓所有 `[[slug]]`、去重填入。對應 design D6。驗證：6.1 全綠；`npx vitest run quiz-parse` 不回歸。依賴 6.1。
- [x] 6.3 (RED) 改寫 `codebus-app/src/components/workspace/QuizAnswering.test.tsx`：刪除/改寫原「incorrect answer shows [← Back to wiki page]」測試；新增 (a) 送出**正確**答案後，explanation 內 `[[slug]]` 以 `WikilinkLink` 呈現（testid `wikilink-<slug>`），點擊 resolvable slug → 呼叫傳入的 `onOpenWikiPage` 並帶該 slug；(b) 送出**錯誤**答案後同樣可點；(c) 斷言**不存在** `quiz-back-to-wiki` testid。並在 `QuizReview.test.tsx` 加：每題 explanation 的 `[[slug]]` 同樣以 `wikilink-<slug>` 呈現。對應 design D6 與 spec `app-workspace` / Quiz Answering and Summary（兩新場景）。驗證：測試先 fail（現為純文字、仍有 back-to-wiki）。依賴 6.2。
- [x] 6.4 (GREEN) `QuizAnswering.tsx`：移除 `onBackToWiki` prop 與 `[← Back to wiki page]` 按鈕；revealed explanation 改用既有 `WikilinkLink`（`codebus-app/src/lib/milkdown-wikilink.tsx`）渲染該題 `sources`，傳入 `pages`（wiki 頁索引）與 `onOpenWikiPage`；新增 `onOpenWikiPage?: (slug: string) => void` prop。`QuizReview.tsx`：同樣把每題 explanation 的 `[[slug]]` 改用 `WikilinkLink` 渲染並接 `onOpenWikiPage`。`QuizTab.tsx`：新增 `onOpenWikiPage` prop 並一路傳給 `QuizAnswering`/`QuizReview`；`Workspace.tsx`：提供該 handler，重用 `onSelectPage` 的「切 `activeTab=wiki` + `useWikiStore` 載入該頁」路徑，並把 wiki 頁索引以 `pages` 傳入 QuizTab。對應 design D6。驗證：6.3 全綠；`npx vitest run` 全綠不回歸；`npm run typecheck` 乾淨。依賴 6.3。

## 7. Resume 還原「最後答過那題的已送出畫面」（design D3 修訂 / Implementation Contract「Resume (revised)」；spec `app-workspace` / Quiz Answering and Summary resume 場景改版）

- [x] 7.1 (RED) 在 `codebus-app/src/components/workspace/QuizAnswering.test.tsx`（必要時 `QuizTab.test.tsx`）新增測試：`initialProgress` 已答 Q1、Q2（共 5 題、in_progress、Q2 selected 存的值）→ 開啟後**顯示 Q2 的已送出畫面**（顯示 Q2 存的選項、verdict、explanation；即 revealed 狀態），**非**跳到 Q3、**非**回 Q1；按 Next → 前進到 Q3（第一個不在 answers 的題號）。對應 design D3（修訂）與 spec `app-workspace`（resume 兩新場景）。驗證：測試先 fail（現行從第一個未答題起始，會直接到 Q3）。依賴 6.4。
- [x] 7.2 (GREEN) `QuizAnswering.tsx`：resume 起點改為 `max(q in answers)` 那題，且初始即 revealed＝以該題存的 `selected` 還原（顯示 verdict + explanation/wikilink）；Next 才前進到第一個未答題；not-started（無 answers）仍 Q1 空白；completed 仍由 QuizTab 路由到 Review（不變）。sidecar schema 不動。對應 design D3（修訂）。驗證：7.1 全綠；既有 3.1/3.2 涉及 resume 的斷言一併更新為新行為且 `npx vitest run` 全綠；`npm run typecheck` 乾淨。依賴 7.1。

## 8. Plan-confirm 畫面文案/relabel/i18n（design D7 / Implementation Contract「Plan-confirm view」；spec `app-workspace` / Quiz Tab Plan-Confirm-Generate Flow 改版）

- [x] 8.1 (RED) 在 `codebus-app/src/components/workspace/QuizTab.test.tsx` 新增/改寫測試：confirm 階段 (a) 說明文字為「將依下列 wiki 頁面出題…」之意且**來自 i18n key**（非寫死英文）；(b) revise 按鈕文字為 `重新規劃`（來自 i18n）、testid 維持 `quiz-revise`；(c) 點 `重新規劃` → 回 topic-input（idle）視圖、`invokedCommands` 不含 `spawn_quiz_*`；(d) `確認` 按鈕文字來自 i18n、行為不變。對應 design D7 與 spec `app-workspace` / Quiz Tab Plan-Confirm-Generate Flow（relabel/i18n 場景）。驗證：測試先 fail（現為寫死英文、按鈕為「改」）。
- [x] 8.2 (GREEN) `codebus-app/src/i18n/messages.ts`：新增 confirm 說明 + `重新規劃` + `確認` 的 keys（`en` + `zh-tw` 各一份）。`QuizTab.tsx` confirm 區塊：說明文字改成「將依下列 wiki 頁面出題，確認後開始生成測驗」之意並列出頁面、改用 `useT()`；revise 按鈕文字 `改`→`重新規劃`（`onClick=reset` 行為不變）；`確認` 文字走 i18n。對應 design D7。驗證：8.1 全綠；`npx vitest run` 全綠不回歸；`npm run typecheck` 乾淨。依賴 8.1。

## 9. 全域回歸（本次 ingest 範圍）

- [x] 9.1 全域回歸 sweep：`cargo test -p codebus-core -p codebus-cli`、`cargo test --manifest-path codebus-app/src-tauri/Cargo.toml`、`cd codebus-app && npx vitest run && npm run typecheck` 全綠（彙總 0 failed）；確認 #2/#3/#1 三項自動測試覆蓋且未回歸既有 quiz-attempt-progress（1–5 群）與 v3-app-quiz 行為。對應 design「In scope (this ingest)」。驗證：四套件彙總 0 failed。依賴 6.4、7.2、8.2。

## 10. Explanation 引用顯示頁面 title、去 `[[ ]]`（design D6 修正：比照 WikiPreview，非 v1 bracketed WikilinkLink）

- [x] 10.1 (RED) 改寫 `codebus-app/src/components/workspace/QuizAnswering.test.tsx` 與 `QuizReview.test.tsx` 既有 wikilink 測試：對某 `[[slug]]`，傳入 `pages[slug].title="Stateless Token Revocation"`，斷言渲染文字為該 **title** 且**不含** `[[` 或 `]]`（resolvable）；查不到的 slug → 顯示純文字 `title ?? slug`、無括號、無法觸發 `onOpenWikiPage`；testid 維持 `wikilink-<slug>`、點擊 resolvable 仍呼叫 `onOpenWikiPage(slug)`。對應 design D6（修正）。驗證：測試先 fail（現 reuse `WikilinkLink` 顯示 `[[slug]]`）。依賴 6.4。
- [x] 10.2 (GREEN) `codebus-app/src/components/workspace/ExplanationText.tsx`：不再 reuse `WikilinkLink`；比照 `WikiPreview` 第 117–153 行邏輯渲染——resolvable→clickable anchor 顯示 `pages[slug].title ?? slug`（無 `[[ ]]`）、`onClick`→`onOpenWikiPage(slug)`；unresolvable→dimmed 純文字 `title ?? slug`；皆掛 `data-testid="wikilink-<slug>"`。對應 design D6（修正）。驗證：10.1 全綠；`npx vitest run` 全綠不回歸；`npm run typecheck` 乾淨。依賴 10.1。
- [x] 10.3 全域回歸 sweep（同 9.1 四套件）彙總 0 failed，確認 title 呈現修正未回歸 #2/#3/#1 與既有行為。驗證：四套件彙總 0 failed。依賴 10.2。

## 11. Precise cursor resume — sidecar 記游標，回到離開時那一格（design D3 final；spec `quiz` sidecar `cursor` + `app-workspace` resume 場景改版）

- [x] 11.1 (RED) 在 `codebus-core/src/verb/quiz_progress.rs` 測試模組新增：`QuizProgress` 加 `cursor: Option<QuizCursor>`（`QuizCursor { q: u32, revealed: bool }`）後 (a) 帶 `cursor` round-trip 序列化/反序列化一致；(b) 一份**省略** `cursor` key 的 JSON → 解析成功且 `cursor == None`（向後相容）。對應 design D3 final 與 spec `quiz`。驗證：測試先 fail（`QuizCursor`/`cursor` 欄位未存在，編譯紅）。依賴 1.2。
- [x] 11.2 (GREEN) `codebus-core/src/verb/quiz_progress.rs`：新增 `QuizCursor { q, revealed }`（serde）＋ `QuizProgress.cursor: Option<QuizCursor>` 標 `#[serde(default)]`；更新 `not_started()`（`cursor: None`）與既有測試 helper；`codebus-app/src/lib/ipc.ts` 的 `QuizProgress` 加 `cursor: { q: number; revealed: boolean } | null`。**不** bump `schema_version`（serde default + unknown-key 容錯已涵蓋；理由寫進 doc 註解）。對應 design D3 final 與 spec `quiz`。驗證：11.1 全綠；`cargo test -p codebus-core` 不回歸（既有 5 個 quiz_progress 測試＋全 core）。依賴 11.1。
- [x] 11.3 (RED) 在 `codebus-app/src/components/workspace/QuizAnswering.test.tsx` 新增測試：(a) `initialProgress` answers Q1–3、`cursor:{q:4,revealed:false}` → 開啟顯示 **Q4 空白**（非 Q3 解答）；(b) `cursor:{q:3,revealed:true}` → 顯示 **Q3 已送出**（存的 selected+verdict+explanation）；(c) **無** `cursor`（legacy）→ 仍走 7.x 的「最後答過題 revealed」（既有 7.1/7.2 斷言維持綠）；(d) 送出某題 → `onPersist` payload `cursor={q:該題,revealed:true}`；按 Next → `onPersist` 再被呼叫且 `cursor={q:下一題,revealed:false}`、`answers` 不變、`status:"in_progress"`。對應 design D3 final 與 spec `app-workspace`（cursor 三場景）。驗證：測試先 fail（現無 cursor 概念、Next 不 persist）。依賴 11.2。
- [x] 11.4 (GREEN) `codebus-app/src/components/workspace/QuizAnswering.tsx`：resume 起點優先用 `initialProgress.cursor`（present→`idx=cursor.q-1`、`revealed=cursor.revealed`、revealed 時 `selected`＝該題存的答案；absent→沿用 `lastAnsweredIndex` legacy）；`onSubmit` 的 `onPersist` payload 加 `cursor:{q:current,revealed:true}`；`onNext` 前進（非結束）時也呼叫 `onPersist`（`answers` 不變、`status:"in_progress"`、`cursor:{q:next,revealed:false}`）。對應 design D3 final。驗證：11.3 全綠；`npx vitest run` 全綠不回歸（含既有 7.x）；`npm run typecheck` 乾淨。依賴 11.3。
- [x] 11.5 全域回歸 sweep（同 9.1 四套件）彙總 0 failed，確認精準 cursor 未回歸 #1/#2/#3/D6 與既有行為。驗證：四套件彙總 0 failed。依賴 11.4。

## Traceability

| Design topic | Tasks |
| --- | --- |
| Non-Goals | 5.1 |
| In scope (this ingest) | 9.1 |
| D1: Immutable md + mutable progress sidecar | 1.1, 1.2 |
| D2: codebus-core owns the sidecar; GUI via two thin IPC commands | 1.2, 2.1, 2.2 |
| D3: Per-submit atomic persistence + resume | 3.1, 3.2 |
| D3 (revised): resume restores last answered question | 7.1, 7.2 |
| D3 (final): precise cursor resume | 11.1, 11.2, 11.3, 11.4, 11.5 |
| D4: History routing by derived status; Review replaces raw md | 4.1, 4.2 |
| D5: Retake = two explicit affordances | 4.1, 4.2 |
| D6: Explanation citations are navigable wikilinks (supersedes the back-to-wiki button) | 6.1, 6.2, 6.3, 6.4, 10.1, 10.2, 10.3 |
| D7: Plan-confirm view clarity + i18n | 8.1, 8.2 |
