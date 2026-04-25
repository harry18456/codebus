## Context

`docs/module-5-generator.md`（488 行）是 Module 5 完整設計文件，已涵蓋：每站 prompt 架構（§三）/ 元件規則（§四）/ 格式驗證（§五）/ plain mode（§六）/ 多檔結構 D-029（§七）/ route.json schema（§八）/ Sanitize 連動（§九）/ 失敗處理（§十）/ SSE event（§十一）/ 測試（§十二）/ 實作順序（§十四）/ D-029 不變式（§十六）。

本 design 不重述 spec 既有內容（依 propose 流程 rules），只記錄 propose 階段必鎖的 4 條 design decisions 的 trade-off + 拒絕方案理由 + risks，為後續 spec 與 tasks 寫入提供決策來源。

依賴 capability：`agent-core`（消費 ExplorerResult）、`usage-tracking`（透過 `llm_chat_provider` 走 audit chain）、`sanitizer`（Pass 1 over output 用 SanitizerEngine）、`sidecar-runtime`（POST /generate endpoint + task registry + SSE channel）。

## Goals / Non-Goals

**Goals**

- 鎖死 propose 階段 4 條 design decisions（Pass 1 / depends_on / output dir / degraded fallback），讓 spec + tasks 寫入有清楚決策來源
- 對齊既有 audit chain 慣例（`<ws>/.codebus/`）與 user-facing 教材路徑慣例（`<ws>/codebus-tutorials/`）
- Risk surface 列清楚：LLM creative output 的 sanitize 缺口、stable id 碰撞、context window 超出、disk 寫入失敗等
- Migration 零破壞 — 新 module、新 endpoint、新 directory，既有 client / fixture 不需改

**Non-Goals**

- 不重述 spec 已涵蓋的設計（per-station prompt 架構 / validator 條款 / route.json schema 等已在 `docs/module-5-generator.md`）
- 不討論 Vue 元件渲染（前端 Stage 6）
- 不討論 Q&A `<QAEntry>` 互動行為（Module 8）
- 不討論 live LLM golden snapshot 接入（D-006 後續打磨期）

## Decisions

### Decision 1: Generator output 過 Pass 1 Sanitizer（YES）

選 **YES** — 每站 markdown + MOC 寫檔前過 `SanitizerEngine.sanitize(content, source=FileSource(path=...))`。

**理由**：
1. **Defense in depth**：LLM input 已 Pass 1（Scanner）+ Pass 2（TrackedProvider）sanitize，但 LLM 是 creative entity，可能 (a) 從 sanitized placeholder 反推「`<REDACTED:secret#0>` 對應的應該是 sk-proj-xxx」（理論上單向不可逆，但 prompt injection 場景需防）(b) synthesize secret-like patterns（合成 fake JWT 結構造成假警報，但更糟是合成 real-looking secret 在訓練資料中見過）(c) echo 未被 source-side scan 涵蓋的內容（LLM 模型 weights 訓練時記住的 public secret / leaked credential）
2. **Audit chain 純度**：「離開 sidecar 的內容都已清理」是 D-022 / D-015 的核心承諾。Generator output 寫到 user-facing `codebus-tutorials/` 等於離開 sidecar 進入使用者可見區，不過 Pass 1 = audit chain 斷一截
3. **成本微秒級**：每站 markdown ~800 字元（spec §三 上限），Pass 1 regex scan 微秒級，per-tutorial 多算 N×幾微秒可忽略
4. **既有 audit chain 自然延伸**：每次 sanitize 命中寫 `<ws>/.codebus/sanitize_audit.jsonl` `pass_num=1` `source.path=codebus-tutorials/{task_id}/stations/sXX-x.md`，與 Scanner 的 Pass 1 同 schema 同 writer 同 audit 檔，**零新 layer 引入**

**拒絕方案**：

- **B. 信任 LLM input 已 sanitize**：過度信任 LLM。input sanitize 不等於 output safe — LLM 創造性是 feature 也是 risk
- **C. Prompt 明指示 LLM 不 echo 原始路徑/字串**：LLM 紀律不可靠（prompt injection / jailbreak / context confusion 都會破紀律）。spec 不該依賴模型行為作 safety 邊界

### Decision 2: Station.depends_on backfill 留 P1 / follow-up（NO for P0）

選 **NO for P0** — Module 5 P0 不解析 MOC graph 反向回填 ExplorerResult.stations[i].depends_on。

**理由**：
1. **Scope discipline**：本 P0 已 ~3d（spec §十四 P0 表）。depends_on backfill 涉及 (a) MOC graph parser（parse station frontmatter 的 `related_stations`）(b) inverted index build（從 child→parents map 變 parent→children）(c) cycle detection（循環依賴是 LLM 常見錯誤）— 是獨立 capability，混入 P0 把 ~3d 拉到 ~5d
2. **介面已留好**：Generator 仍輸出 `route.json` 帶 `related_stations: [s01-x, s03-y]`（從 frontmatter 抓），前端可直接讀 `route.json` 顯示 station 之間關係，**不依賴 ExplorerResult.depends_on 反向回填**
3. **Golden sample 影響可控**：`composite_score(recall, noise, depth=1.0)` 的 `depth` placeholder 暫保 — 與 `golden-sample-baseline` 已 land 的 P0 行為一致，不打破任何既有 baseline
4. **Follow-up 路徑清楚**：未來開 `depends-on-backfill` change 純做 MOC graph parser + ExplorerResult 反向填，不混 markdown generation 邏輯，spec 也乾淨

**拒絕方案**：

- **A. 本 P0 一起做**：scope creep ~2d 換來「`depth` 從 1.0 placeholder 變 真實值」效益很小（金分數 fixture pinning 仍有效，只是 absolute number 從 0.97 變 0.85 之類，不影響 drift guard）

### Decision 3: 輸出根目錄 `<ws>/codebus-tutorials/{task_id}/`（改 spec）

選 **`<workspace>/codebus-tutorials/{task_id}/`**（**改 `docs/module-5-generator.md` L27 `tutorials/` → `codebus-tutorials/`**）。

**理由**：
1. **教材是 user-facing product**：使用者打開 IDE 直接看，需要直觀目錄名
2. **`tutorials/` 太 generic**：使用者既有 repo 可能本來就有 `tutorials/`（如 README、官方教學），撞 namespace 風險高
3. **`codebus-tutorials/` 明確標示來源**：一眼看出「這是 CodeBus 產出，不是手寫」；使用者要 commit 進 repo 或 .gitignore 都好決策
4. **與 audit chain 性質區分**：audit 在 `<ws>/.codebus/`（隱藏目錄）— 是 audit / debug 性質；教材在 `<ws>/codebus-tutorials/`（顯示目錄）— 是 product / consumption 性質。**兩者分目錄反映兩者用途差**
5. **改 spec 成本低**：spec 文件改 1 處 path mention + 連動的 layout 圖，改動 < 30 字元

**拒絕方案**：

- **A. 對齊既有 spec `tutorials/`**：generic name 撞 user folder 風險已述
- **C. 隱藏在 `<ws>/.codebus/tutorials/`**：把 user-facing product 藏進隱藏目錄反 user-facing 哲學 — 使用者看不到不會用，audit 與 product 性質混淆
- **D. 用 task_id 開新目錄 `<ws>/{task_id}/tutorials/`**：每個 task 一個 root 太亂，多 task 跑同 workspace 後 root 散滿 `<ws>`

**連動更新**（屬本 change scope）：
- `docs/module-5-generator.md` L27 / §七 layout 圖 / §八 route.json `file_path` 樣本 / §九 sandbox 描述路徑樣本一律改 `codebus-tutorials/`
- spec scenario / SSE event `file_path` 一律 `codebus-tutorials/{task_id}/...`

### Decision 4: degraded fallback per-station stub + retry quota 3（對齊既有 spec §十）

選 **per-station stub** — `_generate_station(...)` 內 retry loop max 3，failed 後產 minimal markdown（核心文字 + 一個 checkpoint，不出 quiz）寫入 station 檔，frontmatter `degraded: true`，log 寫 `<ws>/.codebus/generator_log.jsonl`。其他 station 不受影響（多檔隔離，D-029 §十六.2）。

**理由**：
1. **Demo 體驗**：整 generate 失敗 = 評審看了一眨眼就沒了。per-station stub 至少展示 N-1 站正常 + 1 站「待重跑」結構，敘事不破
2. **多檔隔離 D-029 §十六.2 已決**：其他站檔案不受影響是 D-029 不變式，本 change 只是把它落地
3. **Retry quota 3 是 LLM call 失敗收斂上限**：超過 3 次重試代表 prompt / context 有結構問題（不是 transient），再 retry 也不會 GREEN，宣告 degraded 是對的
4. **全站 degraded 保護**：route.json 頂層補 `degraded: true` UI 提示「教材品質可能不佳，是否重跑」— 使用者明確知道結果不正常
5. **Disk write 失敗特別處理**：log error 不重試（避免無窮迴圈），station 在 route.json 標 `degraded: true, error: "write_failed"`

**拒絕方案**：

- **B. 整 generate 回滾**：Demo killer 已述
- **C. per-station 跳過不寫檔**：教材結構破裂（`route.json` 列了 5 站但只有 3 個檔），前端 link 變斷鏈，比 stub 更糟
- **D. retry 無上限 + exponential backoff**：與 OpenAI quota / cost 衝突，且 prompt 結構問題無論 retry 多少次都不會通過

## Risks / Trade-offs

- **[LLM 在 sanitize placeholder 反推 secret]** → 風險中；未實際 demo 過。**Mitigation**：Pass 1 over output（Decision 1）+ unit test 加 case 「LLM output 含 `<REDACTED:secret#0>` placeholder 時 Pass 1 不 false-positive 重複 sanitize」+ 後續 live LLM snapshot 階段 review 多份 generated markdown 看實際命中率
- **[stable id slug 中文 title fallback 全是 `s{NN}-station`]** → 風險低；spec §7.4 已寫 `s{NN}-station` fallback，但「全 station 都中文 title」場景下 5 站都會變 `s01-station` / `s02-station` — 雖然 stable id 仍 unique（因為 `{NN}` 不同），但語意失去意義。**Mitigation**：unit test 加「全中文 title fallback shape」case；後續可考慮 LLM-side 強制要求英文 slug 副欄
- **[context window 超 LLM 上限]** → 風險中；spec §十處理為「縮減 related_files 至前 100 行」但「100 行」是經驗值，未必所有 LLM model 都夠。**Mitigation**：用既有 `OpenAIContextLengthError` exception 包裝；fallback 流程進 retry 重新 prompt（而非直接 degraded）
- **[disk full 寫入失敗]** → 風險低；spec §十 已處理為「log error 不重試 + route.json 標 degraded with `error: "write_failed"`」。**Mitigation**：apply 階段測 `OSError` exception path
- **[改 docs/module-5-generator.md 路徑 vs Decision 3 對齊]** → 風險低，純文字改。**Mitigation**：tasks.md 列為一條明確 task，apply 階段一起做
- **[generator_log.jsonl 不在七層 audit chain 清單但落 `<ws>/.codebus/`]** → 風險低；它是 per-Module operational log（與 reasoning_log 性質一致），不是七層 audit chain 之一。CLAUDE.md 七層段不需擴。但 `_audit_paths.py` 加 `_GENERATOR_LOG_FILENAME` 常數讓 path 集中管理
- **[新增 `POST /generate` 與既有 single-slot task registry 互動]** → 風險低；single-slot 規則已 generic（一個 task 在跑時 block 任何新 task creation 含 generate）。**Mitigation**：endpoint test 涵蓋 409 TASK_IN_FLIGHT 行為
- **[`<CodeRef>` 路徑驗證在 P0 還是 P1]** → spec §十四 列 `<CodeRef>` / `<Reveal>` 為 P1。本 P0 validator 仍可寫驗證邏輯但 prompt 模板不要求 LLM 產出 `<CodeRef>`，避免 false-positive issue。**Mitigation**：validator `<CodeRef>` 規則寫在 spec 但 P0 prompt 模板不誘發

## Migration Plan

無 — 純新增 capability + endpoint + directory，既有：
- ExplorerResult schema 不變（Generator 純消費，不擴 Station）
- Tracked / KB / Sanitizer / Tools 既有 capability 不動
- API 既有 endpoint（/scan / /kb/build / /kb/query / /explore）不動
- Test fixture 既有不動
- 使用者 workspace 沒 `<ws>/codebus-tutorials/` 目錄（new tree），首次 generate 自動建立

**Rollback**：git revert work + archive 兩 commit；spec / code / new endpoint / new module 一併還原；既有功能完全不影響。

## Open Questions

無關鍵 open question。次要待 apply 階段拍板的 implementation details：

1. **`STATION_PROMPT_VERSION` 初始版**：date-version `2026-04-25-1` 還是 `v0-p0`？— 對齊既有 prompts 慣例（`JUDGE_PROMPT_VERSION="2026-04-25-1"` / `EXPLORER_PROMPT_VERSION="v0-p0"`），用 date-version 較好（後續 prompt 調整有清楚版本）
2. **Generator 的 `LLMJudge` / `LLMCoverageChecker` 對應角色**：Generator 走 `llm_chat_provider`（`role=CHAT, default_module="chat"`）還是另開一個 role like `GENERATE`？— 不另開新 role（`ProviderRole` enum 4 個固定 D-028），用 `chat` role + `default_module="generate"` 區分（與 `usage-tracker-dedup` 慣例一致）
3. **MOC `tutorial.md` 是否寫 frontmatter**：spec §7.1 寫「可不帶；若要帶限於六欄」— P0 預設不寫（簡化），需要時 P1 加

這三條 apply 階段 first-task 拍板即可，不阻塞 propose。
