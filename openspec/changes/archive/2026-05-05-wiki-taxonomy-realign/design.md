## Context

Phase 1 codebus 自我加碼了若干 Karpathy 真本沒有的 wiki taxonomy 強制規定。Karpathy 真本（`https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f`）：
- Named files: `index.md`、`log.md`、schema doc
- Folders: `raw/`、`wiki/`（**flat、無強制 sub-folder**）
- Page categories: 提到 entities / concepts / sources 為**例子**（不是強制 enum）
- 明文：「everything mentioned above is optional and modular」

Phase 1 codebus 自我加碼：
- `wiki/overview.md` 列入 SPECIAL_FILES，schema §3 強制每 goal「rewrite each run」
- `wiki/goals/<slug>.md` per-goal reading guide，schema §4.1 step 7 強制每 in-scope goal 寫一份
- `wiki/{concepts,entities,modules,processes,synthesis}/` 五個 hard-enforced type folder，lint warn 對 folder/type mismatch
- Lint catalog 含 goal guides 作為合法 wikilink target，且對 goal guides 做 body wikilink scan

uv repo spike（4 goal + 1 query，14 knowledge pages）的證據：
- 4 個 goal guide 沒一個被回讀；narrative 內容跟 log.md 對應 entry 重疊
- overview.md 每 goal 被覆蓋，最後一份是 last goal 視角的 snapshot 而非 cumulative
- spike 中觸發 5 個 lint warn 是 markdown table escape 的 false positive（已 known，獨立 backlog 處理），跟本 change 不直接同 root cause 但同樣源於 lint over-spec
- `module` / `process` 兩個 type 觸發率 4/4 與 2/4，code domain 延伸合理；`synthesis` 0/4 但其 page-type 概念對應 Karpathy 的「Overview/synthesis pages」，保留有用

「保留 type enum、移除 folder hard-enforce」這個細節需要 design 釐清，不是純粹「全砍」。

## Goals / Non-Goals

### Goals

- 把 schema、lint、init、layout 對齊到「Karpathy 為基線 + code domain 必要延伸（module/process）」
- 移除 spike 已驗無實用價值或設計錯誤的強制（goals/、overview.md 強制 rewrite、folder/type mismatch warn）
- 保留 spike 已驗有用的延伸（5 type enum、5 type folder 預建）
- 為現有用戶 vault 提供清楚的 forward-compatibility 行為（不破壞既有檔案，但不再 special-case）

### Non-Goals

- **不寫 vault migration script**：codebus 不主動清理現有 vault 的 `wiki/goals/` 目錄、`overview.md`、folder/type mismatch 檔。User 可自行刪
- **不解 page-merge bias**：spike REPORT backlog #3 是另一系列問題（schema 推 update vs new page 的傾向），跟 taxonomy 縮編解法不重疊
- **不解 lint markdown false positive**：inline-code / table-escape 的 regex bug 由獨立 change 處理（建議名 `lint-markdown-aware-scan`）
- **不引入新 page type**：5 type enum 不變

## Decisions

### Decision 1: `wiki/goals/<slug>.md` 完全移除（不 deprecate、不留兼容層）

選擇：徹底移除，不做 deprecation 期。

**Rationale**:
- Spike 證明 goal guide 的核心 use case（re-run detection、goal narrative archive）已被其他機制承擔：re-run 真正穩定的 anchor 是 `page.frontmatter.goals[]` array 而非 file existence；narrative 收進 log.md 對 reader 更線性。
- 留兼容層（如「lint 仍 catalog goal guides 但不強制寫」）反而留 phantom path：未來 reader 看到 goals/ 下舊檔會困惑為何 schema 不提。
- Existing user vault 的 `wiki/goals/<slug>.md` 在新規定下變 wiki/ 下層的「unknown .md」，**lint 會 warn `page lives in wiki/<type>/ folder` 規定不符**——這個 warn 對 user 是「該刪了」的明確信號，不是 silent break。

**Alternatives considered**:
- A. Soft deprecate（schema 不提、lint 仍 catalog）：留 phantom path、user 不清楚 status
- B. Auto-migrate（codebus 跑時主動 mv `wiki/goals/*.md` → `wiki/log.md` append）：非預期 file mutation，違反 codebus「不主動改 user 既有 wiki 內容」原則
- C. **本選擇 — clean break**：明確、可預期、warn-as-signal

### Decision 2: `overview.md` 不再 SPECIAL_FILES，overview 性質改用 `synthesis/<slug>.md`

選擇：從 lint 的 SPECIAL_FILES 移除；schema §3 把「repo overview」描述改成 synthesis page type 的一個 use case（agent 自決建立時機）。

**Rationale**:
- Karpathy 真本 overview/synthesis 是 page type 不是 named file。
- Spike 證明「rewrite each run」會讓 overview 變成 last-goal-snapshot 而非 cumulative。改 synthesis 後由 agent 在累積 5+ page 才考慮 synthesize，符合 Karpathy 原意「cross-cutting summary that integrates multiple pages」。
- 對既有 vault 的 `wiki/overview.md`：lint 不再 SPECIAL_FILES 但也不會 warn it as misplaced（因為它在 wiki/ 根但 SPECIAL_FILES 規則不再適用 + wiki/ root 走 §4 規則，§4 仍會把它當 `page lives in wiki/ root` warn）——這跟 goals/ 同模式：clean break + warn-as-signal。

**Alternatives considered**:
- A. 保留 overview.md 但改 semantic 為「append section per goal」：仍是 named file 強制，違反 Karpathy 簡化精神
- B. 移到 wiki/synthesis/overview.md（強制建）：仍是強制，只是換 folder
- C. **本選擇 — 完全交給 agent 判斷**：對齊 Karpathy「optional and modular」精神

### Decision 3: 5 type folder 保留 + folder/type mismatch warn 移除

選擇：init 仍 mkdir 5 個 type folder（Obsidian sidebar UX）；frontmatter `type` enum 仍是 5 種（schema 仍 instruct agent 寫 module type page 放 modules/）；但 lint **不再** warn folder/type mismatch。

**Rationale**:
- 5 type folder 對 Obsidian sidebar 是好的 organization hint（spike 中 4/5 folder 有用）。預建是低 cost 高 UX value。
- frontmatter type enum 是 metadata contract（page-merge / index.md 分類用）—— 不能弱化。
- folder/type mismatch warn 是 phase 1 自我加碼。Karpathy 沒這要求。實務上：agent 偶爾把 ambiguous page 分到「次佳」folder，原本 lint 會 warn — 但這個 warn 沒實質壞處（檔仍正常 wikilink、Obsidian 仍 render）。soft 化後 lint signal 純度上升。

**Alternatives considered**:
- A. 完全移除 5 folder mkdir：Obsidian sidebar 變 cluttered，type 為 hint 變不清晰
- B. 保留 5 folder + 保留 lint warn：對齊現有但違反 Karpathy 簡化，且 spike 證明 mismatch 不痛
- C. **本選擇 — folder 留作 UX hint，lint 軟化**：UX/contract/lint 三層分離

## Risks / Trade-offs

- **Existing vault overview.md 跟 goals/ 變 lint warn 來源** → user 看 warn 會困惑「為什麼以前可以」 → mitigation：在 commit message + 新 README/schema comment 明文寫此 breaking change；lint warn 訊息也可加「post-2026-05-05 schema：X 已不再支援」hint（屬於 nice-to-have，非本 change 必交付）
- **schema CLAUDE.md 改後既有用戶 vault 不會自動更新**（per CLAUDE.md L113 既存規定：runInit preserves existing schema）→ user 必須手動覆寫 `.codebus/CLAUDE.md` 或 `rm -rf .codebus/` 重 init 才看到新規定 → mitigation：CLAUDE.md 自身已記錄此行為，本 change 不需新解
- **`tests/commands/goal.test.ts` SabotageGoalsProvider 失效** → 該 test 透過破壞 wiki/goals/ 觸發 lintWiki throw 來驗 lint:null fallback。goals/ 移除後此 sabotage vector 消失 → mitigation：找替代 vector（譬如把 wiki/index.md 替換成 dir）或將 sabotage logic 改 mock lintWiki 直接 throw
- **`process` page type 在缺乏 sequential workflow 的 repo 上仍預建空目錄** → 一般 repo 都有流程性 code（API call / lifecycle），spike 已驗 50% 觸發率，可接受
- **Synthesis page 在小 vault 仍 0 觸發** → 已知（spike 4 goal 0/4），symptom 不在 taxonomy 縮編可解，屬 schema 推導 (page-merge bias) 範疇，留下個 change

## Migration Plan

本 change 為 schema + lint + init 行為變更，**對 codebus 自身的 deploy** 跟一般 feature change 無異（commit + 用戶 npm install 升級）。對 **existing user vaults** 的影響：

1. 新 codebus version 跑 `--check` 對舊 vault：
   - 若有 `wiki/goals/<slug>.md` → 變 wiki/<sub>/ 規則之外的位置 → §4 wiki/ root walk 不會抓到（goals/ 不是 root），實際走的是 §1 type folder 掃 — goals 不在 5 type folder 名單，**完全不被掃** → silent ignore（不 warn 也不 catalog）。User 自行決定刪不刪
   - 若有 `wiki/overview.md` → 從 SPECIAL_FILES 移除後，§4 wiki/ root walk 會把它當「root .md 非 special」 → emit `page lives in wiki/ root` warn → **明確信號要 user 處理**
   - 若有 folder/type mismatch（如 `concepts/foo.md` frontmatter type=module）→ 不再 warn → silent forgive

2. 新 codebus version 跑 `--goal` 對舊 vault：
   - schema CLAUDE.md 內容不會自動更新（per existing init 行為），user 必須手動 `rm .codebus/CLAUDE.md` 後 re-init 才用到新 schema
   - 即使用舊 schema，新 lint/init 行為已生效 → 舊 schema 仍指示 agent 寫 goal guide → guide 寫在 `wiki/goals/` → 但 init 不再 mkdir 該 dir → **agent 仍會嘗試 mkdir + 寫**（cwd=`.codebus/` 內仍有 Write 權限）→ goal guide 仍能寫成功，只是 lint 不會 catalog 它。**漸進式 silent migration**：用戶不重 init 也不會壞，只是慢慢失去 goal guide 的 lint coverage

3. **codebus repo 自己**有沒有 vault？沒有（dogfood vault 留在使用者 repo 那邊）。本 change 不需 self-migration。

不寫自動 migration script。Spike 用的 `D:/side_project/uv/.codebus/` 是測試殘留，使用者自己決定要不要 `rm -rf` 重來。

## Open Questions

- (none — 上面 3 個 decision 各自有 alternatives 列出，已收斂)
