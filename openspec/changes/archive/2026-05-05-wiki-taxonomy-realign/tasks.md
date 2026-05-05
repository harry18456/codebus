## 1. Schema content rewrite

更新 agent 的系統 prompt（`src/schema/claude-md.ts`）對齊新 taxonomy。`tests/schema/claude-md.test.ts` 鎖住關鍵 phrase，TDD 先動 test。

- [x] 1.1 更新 `tests/schema/claude-md.test.ts`：移除「overview.md 為 named file」「§4.1 step 7 goal guide write」「wiki/goals/<this-slug>.md 為 re-run signal」「5 type folder 強制」相關的 assert；新增「step 6 log 含 narrative coverage」「§3 把 overview 描述為 synthesis page type」「frontmatter type 為 5 enum」「folder 為 organizational hint」的 assert。tests 此時應 fail（schema 還沒改）
- [x] 1.2 更新 `src/schema/claude-md.ts` §3 Wiki Structure：移除把 `overview.md` 列為 named root file 的描述；移除 `wiki/goals/<slug>.md` 的 nav-file 描述；overview 性質的 page 描述改 routed 到 `wiki/synthesis/<slug>.md`（agent 自決時機，累積 5+ page 後考慮）；5 type folder 措辭從「mandatory taxonomy」改「organizational hint — frontmatter type 是 authoritative metadata，folder 是 Obsidian sidebar UX」
- [x] 1.3 更新 `src/schema/claude-md.ts` §4.1 Workflow per Goal (Ingest)：移除 step 7 「Guide: write wiki/goals/<slug>.md」；step 6 「Log」擴充——從「append a line to wiki/log.md」改「append a chronological entry to wiki/log.md including: covered pages（[[A]], [[B]]）+ 建議閱讀順序 + 本次 goal 重點 narrative（取代原 goal guide 角色）」；step 5 不變
- [x] 1.4 更新 `src/schema/claude-md.ts` §4.0.1 STOP rules：移除 「No `wiki/goals/<slug>.md`」 與 「No `wiki/overview.md` update」 兩條（已不適用）；保留 No log/index/type-folder pages 三條。同時 §4.0 Pre-flight 移除 `wiki/goals/<this-slug>.md` 作為 re-run anchor 的 bullet（goal-guide 已移除，不再是 anchor）
- [x] 1.5 更新 `src/schema/claude-md.ts` §7 WikiLinks Convention：把 special-files-as-target 段從 `[[overview]] / [[index]] / [[log]]` 縮為 `[[index]] / [[log]]`。§11 / §12 原本就只 reference index.md（沒 overview/goals 提及），不需動
- [x] 1.6 跑 `npm test -- tests/schema/claude-md.test.ts` 確認 test pass（14/14）

## 2. Lint code + test changes

`src/core/wiki/lint.ts` 是單一檔，為避免 race condition 不並行；`tests/core/wiki/lint.test.ts` 跟它配對 sequential。本 §2 全段對應 design.md decision 2 (`overview.md` 不再 SPECIAL_FILES，overview 性質改用 `synthesis/<slug>.md`) + decision 3 (5 type folder 保留 + folder/type mismatch warn 移除)，並落地 4 個 spec MODIFIED requirement：「Lint emits warnings for structural and Obsidian-compatibility violations」、「Wikilink catalog includes nav files and goal guides as valid targets」、「Lint scans body wikilinks in nav files and goal guides」、「Lint result schema and report format」。

- [x] 2.1 更新 `tests/core/wiki/lint.test.ts`：(a) 刪除「flags WARN for folder/type mismatch」、「flags WARN for missing special files」（針對 overview）、「flags WARN for broken wikilink in overview.md body」、「resolves wikilinks pointing at existing special files」（overview 部分需收窄至 index/log）、「flags WARN when wikilink targets a missing special file」（overview 部分）、「resolves [[goal-slug]] wikilink」、「flags WARN for broken wikilink in goal guide」7 個 test。(b) 把「counts all existing nav files」跟「does not count missing special files in navFilesScanned」改為只算 index.md + log.md（不含 goal guides）。(c) 新增 5 個 test 對應新 spec scenarios：「does not flag folder/type mismatch」、「does not flag missing overview.md」、「[[goal-slug]] now flagged broken even when goals/<slug>.md exists」、「navFilesScanned ignores files in wiki/goals/」、「clean run reports `N pages + 2 nav files scanned`」。tests 此時應 fail。對應 spec MODIFIED requirements: 「Lint emits warnings for structural and Obsidian-compatibility violations」/「Wikilink catalog includes nav files and goal guides as valid targets」/「Lint scans body wikilinks in nav files and goal guides」/「Lint result schema and report format」
- [x] 2.2 更新 `src/core/wiki/lint.ts`：(a) `SPECIAL_FILES` 從 `['overview.md', 'index.md', 'log.md']` 改為 `['index.md', 'log.md']`。(b) 刪 §3 page-walk 段裡 folder/type mismatch 的 issue push（保留 frontmatter parse + related[] 兩條 error 規則）— 落實 design decision 3「5 type folder 保留 + folder/type mismatch warn 移除」。(c) 刪 §1c 整段（goalsDir readdir + pageSlugs.add 迴圈、`goalGuideFiles` 收集）— 落實 design decision 1「`wiki/goals/<slug>.md` 完全移除（不 deprecate、不留兼容層）」。(d) 刪 §6 整段（goal guides body scan loop）。(e) `navFilesScanned` 計數自動隨 §6 移除而調整為僅特殊檔
- [x] 2.3 跑 `npm test -- tests/core/wiki/lint.test.ts` 確認 test pass（24/24）

## 3. Init + Layout 移除 wikiGoals

兩個 test 檔不同路徑，可 [P] 並行。對應 src 檔 sequential（依賴 layout interface）。

本 §3 落地：
- spec MODIFIED requirement: **Initialize .codebus/ vault structure under user repo** — 更新 vault-init scenario「Fresh init creates all expected paths」移除 `wiki/goals/`
- design **decision 1: `wiki/goals/<slug>.md` 完全移除（不 deprecate、不留兼容層）** — codebase 跟 layout 介面層面的落地，跟 §2 lint 層面落地互補
- design **decision 2: `overview.md` 不再 SPECIAL_FILES，overview 性質改用 `synthesis/<slug>.md`** — 跟 §1 schema 層、§2 lint 層共同落地此 decision（init/layout 不需直接動，但對應的 vault 不會再為 overview.md 做特殊安排）
- design **decision 3: 5 type folder 保留 + folder/type mismatch warn 移除** — init 仍預建 5 type folder（保留部分），跟 §2 lint mismatch warn 移除（移除部分）共同落地
- Non-Goals「不寫 vault migration 工具」對應 §3.4 init.ts 修改後對 existing user vaults「漸進式 silent migration」的行為（per design.md Migration Plan）

- [x] 3.1 [P] 更新 `tests/core/vault/layout.test.ts`：移除對 `wikiGoals` 路徑欄位的 expectation（包含 layout 物件 shape assertion 跟跨平台 regex check）。test 此時應 fail
- [x] 3.2 [P] 更新 `tests/commands/init.test.ts`：移除「init creates wiki/goals/ directory」的 assertion；保留其他 5 type folder 的建立 assertion。test 此時應 fail
- [x] 3.3 更新 `src/core/vault/layout.ts`：從 `VaultPaths` interface 移除 `wikiGoals: string`；從 `vaultPaths()` 回傳物件移除 `wikiGoals: join(wiki, 'goals')` 欄位；TypeScript 編譯應自動標出所有 consumer
- [x] 3.4 更新 `src/commands/init.ts`：移除 `await mkdir(p.wikiGoals, { recursive: true })` 那行（init 不再預建 goals/ 目錄；其他 5 type folder mkdir 保留）。此 task 直接落地 spec requirement「Initialize .codebus/ vault structure under user repo」的修訂 scenario「Fresh init creates all expected paths」
- [x] 3.5 跑 `npm test -- tests/core/vault/layout.test.ts tests/commands/init.test.ts` 確認 test pass（10/10）

## 4. goal.test.ts SabotageGoalsProvider rework

原本透過 `wiki/goals/` 變檔觸發 lintWiki throw，goals/ 移除後此 vector 失效，需替代。

- [x] 4.1 更新 `tests/commands/goal.test.ts` 的 `SabotageGoalsProvider`：實作上不採 wiki/concepts 變檔（會讓 enrich/flagStale 先 readdir 拋）；改採 wiki/ 整個 dir 變檔——這樣 enrich/flagStale 對 wiki/<type>/ 的 existsSync 全 false 直接 skip 不報錯，但 lintWiki 對 wiki/ root 自己的 existsSync 為 true、readdir(wikiRoot) 在 §4 拋 ENOTDIR，被 try/catch 吞，lint:null 落地、autoCommit 仍跑。注釋已更新說明新 vector
- [x] 4.2 跑 `npm test -- tests/commands/goal.test.ts` 確認 5 個 test 全 pass（含修改後的 SabotageGoals 案）— 5/5 ✓

## 5. Acceptance

- [x] 5.1 跑 `npm test` 全程 pass。**26 file / 160 tests pass**。Δ from baseline 155: +5 (全 schema test 新增；lint test 刪+加均衡 0；init/layout/goal 不變)。schema test 從 9 變 14；lint test 維持 24（刪 5 加 7 modified 3）；init/layout/goal 各 9/1/5 不變
- [x] 5.2 跑 `node dist/cli.js --check --repo D:/side_project/uv` 對 spike vault。Coverage 從「12 pages + 6 nav」變「14 pages + 2 nav」（goal guides 不再算 nav）。**Warning 從 6 變 10**：overview.md 從 missing-special warn 改為 wiki/ root warn（−1 +1）；多 4 條 index.md body broken-wikilink 指向廢棄的 goal-guide slug（design Migration §1 預測的明確信號）；5 個 uv-resolver table-escape false positive 維持不變（獨立 change 解決）。所有 delta 全部對齊 design Migration Plan 預測
- [x] 5.3 跑 `spectra validate wiki-taxonomy-realign` 確認 valid ✓
- [x] 5.4 跑 `spectra analyze wiki-taxonomy-realign --json` 0 critical+warning ✓
- [x] 5.5 commit：訊息標明 (a) BREAKING schema/lint/init 行為變更、(b) 不寫 migration 工具、(c) 對 existing user vault 的明確 forward-compat 行為、(d) 對應 spike REPORT 第 #3 / 第 #4 wishlist 之外的獨立 follow-up
