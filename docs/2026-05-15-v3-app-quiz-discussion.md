# v3-app-quiz — pivot + 三件套架構討論

> 2026-05-15 roadmap-level 討論紀錄（pre-spike）。觸發於 C `v3-app-workspace-goal` archive (2026-05-14) 後、E `v3-app-quiz` 動工前 user 提問「該不該獨成新指令 / skill」+「先 CLI 確認可行」+ `+ New quiz` 流程 pivot。
>
> 上游：`docs/v3-app-roadmap.md` §Sequence row E、`docs/2026-05-11-app-ux-flow-design.md` §4.5（quiz UX 設計）、`docs/2026-05-13-chat-verb-discussion.md`（chat verb spike 結論 + 三件套 pattern 來源）。

## TL;DR

**v3-app-quiz 走三件套**：`codebus_core::verb::quiz` library + `codebus-quiz/SKILL.md` bundle + `codebus-cli/src/commands/quiz.rs` CLI thin wrapper。對齊既有 chat / goal / query / fix verb pattern。

**動工前先 spike**（同 chat verb 模式）：四個 shell-level spike 驗 prompt + format + scope-enforce 可行性，結果回饋 SKILL.md 草稿與 prompt schema，pass 才進 propose。

**`+ New quiz` flow pivot**：從原「pick page from list → 1-hop scope」改成「**user 輸入主題 → AI 規劃 wiki scope → user 確認清單 → 出題**」，全程 read-only on `wiki/`、**禁讀 raw/**。Two-spawn 設計（規劃 + 出題），**兩階段都 live stream agent 思考過程**（reuse goal C 的 timeline component）。

**`[Quiz me on this]`（wiki preview 觸發點）保留**：已知 target page、跳過規劃直接 1-hop（與 §4.5.2 一致）。兩條 trigger 在「答題畫面」之後 downstream 一致。

**Storage / retry 行為**：每次出題產獨立 md（`<vault>/.codebus/quiz/<page-or-topic-slug>/<timestamp>.md`），retry 用**同主題 + 同 wiki 範圍 + 新檔不覆蓋**；retry spike 要驗「題目不重複」。

**Roadmap E 維持單一 change 不拆**。Scope 從原「Quiz flow + 觸發點 + md 持久化」擴充為加上：(a) goal-input planning sub-flow、(b) two-spawn + confirm gate、(c) live stream during both spawns、(d) raw/ scope enforcement、(e) generation event log surface (`[看過程]` button)。

## 觸發點（user 提出的問題）

1. v3-app-quiz 該獨成新指令 / skill 嗎？→ **是，走三件套**
2. 先從 CLI 確認可行？→ **是，4 個 shell spike 先跑**
3. `+ New quiz` 改成輸入目標 → AI 規劃；不讀 raw → **採用**
4. retry 是「同主題 + 同 wiki + 新檔不覆蓋」→ **採用**
5. AI 規劃過程要全程顯示（像 goal）+ 中途要 user 確認 → **two-spawn + confirm gate + 兩階段 live stream**

## 三件套（同 chat / goal / query / fix）

| 理由 | 細節 |
|---|---|
| v3 一貫 CLI-first | foundation + A + B + chat 都是這個 pattern；無理由打破 |
| Quiz spawn LLM 本質與 4 verb 同模 | 都是 spawn claude CLI + read-only sandbox + stream events |
| SKILL.md 要 CLI 調過 GUI 才能消費 | 在 terminal 跑 quiz spawn 觀察 agent → 改 SKILL → 收斂 → GUI 才接 |
| CLI quiz 也是真實 use case | dev workflow 在 terminal 跑 quiz 自我驗證 wiki 可用 |

對既有 verb 結構的對齊（不破例）：

- `codebus-core/src/verb/quiz.rs` + `QUIZ_TOOLSET: &[&str] = &["Read", "Glob", "Grep"]`（同 query toolset 層級）
- `codebus-quiz/SKILL.md` 新 bundle（mirror 部署 `<vault>/.codebus/.claude/skills/codebus-quiz/`）
- `codebus-cli/src/commands/quiz.rs` **thin wrapper**（與 chat REPL 不同，quiz 是 one-shot；對齊 query / goal / fix 既有 thin wrapper pattern）

## Two-shot：生成 LLM + client-side 評分

§4.5 UX 鎖死兩階段：

| 階段 | 是否 spawn LLM | 機制 |
|---|---|---|
| 生成 quiz md | ✅ spawn | LLM 拿 wiki page context → 產 5 題 + 4 選項 + correct + explanation md |
| 答題評分 | ❌ 不 spawn | Frontend 比對 user 選的選項 vs quiz md frontmatter 裡的 `correct` 欄位 |

評分 client-side = **無 free-text 答案**（v1 lock）。未來若要支援 free-text 評分（v2 polish）才會升級成 REPL-style。

## `+ New quiz` pivot 詳細

### 原 §4.5 design（pre-pivot）

`+ New quiz` button → 跳 page picker（列所有 wiki pages）→ user 點一頁 → prep screen 顯示「target + 1-hop wikilinks」清單 → user `[Generate]` → 出題。

### Post-pivot design

```
+ New quiz button
    │
    ▼
┌───────────────────────────┐
│ 輸入主題（goal text）：     │
│ 「想了解 auth 是怎麼運作的」 │
└─────────────┬─────────────┘
              ▼
        [Spawn 1: 規劃]
        ├─ live stream agent 探索 wiki/
        ├─ tool_use 顯示「→ Glob wiki/**.md」「→ Read auth-middleware.md」
        └─ emit scope marker → 顯示 wiki 清單
              │
              ▼
┌───────────────────────────┐
│ AI 規劃用這 3 頁出題：       │
│  📄 modules/auth-middleware│
│  📄 concepts/jwt-token...  │
│  📄 processes/login-flow   │
│                           │
│           [改] [確認]      │
└─────────────┬─────────────┘
              ▼ user 按「確認」
        [Spawn 2: 出題]
        ├─ live stream agent 讀 pages
        ├─ tool_use 顯示「→ Read jwt-token-lifecycle.md」
        └─ quiz md 寫入 .codebus/quiz/<slug>/<ts>.md
              │
              ▼
       答題畫面（Q1/Q2/.../Q5）
```

Stream UI **reuse goal C 的 mini-stream component**（C archive 2026-05-14）。Propose 階段第一個 task = inspect 該 component 是否 reusable；若不是、layer 2 stream 範圍縮減（見 §紀錄與 surface）。

### 兩條 trigger 在 downstream 匯流

| Trigger | 規劃階段 | 出題階段 | 答題 / summary / history |
|---|---|---|---|
| `[Quiz me on this]`（wiki preview button） | 跳過 — target page 已知、1-hop scope 自動 | 與 sidebar 路徑相同 | 完全一致 |
| `+ New quiz`（sidebar） | 有規劃 spawn + confirm gate | 與 wiki preview 路徑相同 | 完全一致 |

## 「不讀 raw/」enforcement 層級

claude CLI sandbox = tool-level 不 path-level（chat-verb spike ❶❷❸ 驗證結論：`--tools Read,Glob,Grep` 給 read access 但**沒有 native path allowlist**）。三條 enforcement 路徑：

| 路徑 | 機制 | 採用 |
|---|---|---|
| **SKILL prompt invariant** | `codebus-quiz/SKILL.md` 寫死「Read scope: `wiki/` only, MUST NOT read raw/」 | ✅ 必用 |
| **Library 層 hook tool_use events** | `run_quiz` 監聽 stream-json `tool_use.input.path`，看到 `raw/` 立即 cancel + RunLog `outcome=scope_violation` | 條件採用（視 spike ❽ 結果） |
| Spawn cwd = `<vault>/wiki/` | 把 wiki 當 root | ❌ 不採用（wikilinks resolver + `wiki/index.md` 路徑會壞） |

Spike ❽ 結果決定是否上 library hook：若 SKILL prompt 在 4-5 scenario 足以擋住 agent 偷讀、就不必加 library hook（先 prompt-only ship）。

## 紀錄與 surface（三層）

### Layer 1: events.jsonl（白拿、必做）

每個 quiz spawn 自動寫 RunLog + events.jsonl，繼承 B `v3-run-log-events` 基建。**0 額外實作成本**。

### Layer 2: live stream during spawn

兩 spawn 進行中 GUI 顯示 agent thinking + tool_use stream — **reuse goal C 的 mini-stream component**。若 component 不 reusable 則退到「spinner + 階段標籤」，等 polish-ship 階段再補。

### Layer 3: `[看過程]` button on history row

Quiz history row 上加按鈕 → 開既有 events.jsonl 對應的 timeline view（事後也能翻看）。

### Quiz md frontmatter

```yaml
---
quiz_id: 2026-05-15T14-30-22
trigger: ai_planned          # 或 wiki_preview
topic: "想了解 auth 是怎麼運作的"   # ai_planned 才有
target_page: null            # wiki_preview 才有
planned_pages:
  - modules/auth-middleware
  - concepts/jwt-token-lifecycle
events_log: .codebus/log/events-2026-05-15T14-30-22.jsonl
generation_token_usage: { input: 12500, output: 850 }
---
```

`events_log` pointer 無條件加 — 不管 layer 2 / 3 做不做、未來都能補上 surface。

## Storage / retry

```
<vault>/.codebus/quiz/
├── auth-middleware/                          ← wiki_preview trigger（page-keyed folder）
│   ├── 2026-05-11T14-30-00.md
│   └── 2026-05-11T16-45-22.md                ← retry attempt
└── topic-auth-understanding/                 ← ai_planned trigger（topic-slug folder）
    └── 2026-05-15T14-30-22.md
```

Retry = 同 topic（或 page）+ 同 wiki 範圍鎖定 + 新 timestamped md。Spike ❿ 驗「同範圍下 retry 題目不重複」。

## Spike 列表（CLI shell-level）

| # | Spike | 失敗影響 |
|---|---|---|
| ❼ | Agent 從 goal text 能否規劃合理 wiki page list（4-5 scenario：曖昧 goal / 具體 goal / no-match goal） | scope planning 不穩 → 退回原「user 手動 pick from wiki list」 |
| ❽ | Agent 在 SKILL prompt 約束下會不會偷讀 raw/（5 scenario 自由跑、看 tool_use trace） | 全試讀 → 必須加 library tool_use hook |
| ❾ | LLM 輸出 quiz md schema 穩定性（5Q × correct × explanation × frontmatter） | 不穩 → 加 schema retry loop 或改 structured output |
| ❿ | 同範圍 retry 多樣性（同 topic + 同 pages，跑 3 次題目重複率） | 重複 → prompt 加 negative context「avoid previous Q stems」 |

❶❷❸❺❻ chat-verb 既有 spike 結論直接 reuse（`--resume` 不需要、session_id 取得、stream-json parse、SKILL inject 機制）。

## Roadmap 影響

E `v3-app-quiz` **單一 change 不拆**，scope 從原本擴充：

| 維度 | 原 | Post-pivot |
|---|---|---|
| 觸發點 | wiki preview + sidebar pick-page | wiki preview + sidebar goal-input |
| Sidebar 流程 | pick page → 1-hop | input goal → AI plan → confirm → generate |
| Spawn 數 | 1（生成） | 2（規劃 + 生成） |
| Scope enforce | 隱含 wiki/ only | SKILL prompt（必用）+ 視 spike 結果加 library hook |
| Stream visibility | 不要求 | 兩 spawn 全程 live stream（reuse goal component） |
| 三件套 | 隱含 | 顯式（verb + SKILL + CLI thin wrapper） |

順序不變（D → E → F）。E 內部分前後 phase：

```
spike day (❼❽❾❿) → propose → apply → archive
```

## 動工順序

```
今天         → 本 discussion doc ship
之後         → spike day（半天 ~ 一天，跑 ❼❽❾❿ 4 個 shell spike）
spike 後     → /spectra-propose v3-app-quiz（帶 spike 結果進 propose）
propose 後   → /spectra-apply v3-app-quiz
ship 後      → F v3-app-polish-ship
```

## 結論

| 問題 | 答案 |
|---|---|
| Quiz 該獨成新指令 / skill？ | 是。三件套（verb + SKILL + CLI thin wrapper）；CLI 是 thin wrapper、不是 REPL（不像 chat） |
| 先 CLI 確認可行？ | 是。4 個 spike（❼❽❾❿）shell-level 跑、結果回饋 SKILL.md 草稿 |
| `+ New quiz` 流程 | Two-spawn agentic：goal-input → AI 規劃 wiki → user confirm → 出題；兩階段全程 live stream |
| `[Quiz me on this]` 流程 | 保留 — 跳過規劃、直接 1-hop 出題（target page 已知） |
| 不讀 raw/ enforce | **SKILL prompt-only**（spike ❽ PASS — 9/9 zero raw access；library hook 降為 spec fallback note，v1 不做） |
| Retry 行為 | 同 topic + 同 wiki + 新 timestamped md、不覆蓋舊檔；**retry = 純 re-spawn、不做 diversity 處理（product 決定接受隨機，spike ❿ 證實小 wiki 廣度上限無法解）；spec 須明寫「不保證新題」** |
| 紀錄 / surface | events.jsonl（白拿）+ live stream during spawn（reuse goal）+ `[看過程]` button on history row |
| Config | `quiz.default_length` 搬離 `app.*` → 共用 `quiz.*`（CLI+app 共讀）；`pass_threshold` 留 `app.*`；v3-app-quiz 顯式 own 此 archived foundation migration |
| Roadmap 影響 | E 單一 change 不拆；spike 不算入 E 工作量；**E scope 增 archived foundation config migration（supersede app-shell AppConfig Namespace Isolation）** |

## Config 決策：`quiz.default_length` 搬離 `app.*`（2026-05-15、user 拍板 A）

### 決策

- **`default_length` 搬出 `app.*`** → 新共用 namespace `quiz.*`（`codebus-core/src/config/quiz.rs`，CLI + app 共讀，pattern 對齊 `lint.*` / `pii.*` / `log.*` / `claude_code.*`）
- **`pass_threshold` 留在 `app.*`** — 答題評分是 client-side UI 概念，CLI `codebus quiz` 只產題沒有 pass/fail 畫面，不需要它

### 為什麼

Foundation 把 quiz length 放 `app.*` 是基於「quiz 是 app-only feature」假設；quiz 確立三件套後 `codebus quiz` CLI 是真實 use case（有持久題數偏好需求），假設失效。搬遷 = 修正過時決策（實際 second impl 存在，非 speculative abstraction）。Single source of truth 才不會「app 設 5、CLI 跑 3」。

### 動到的 archived artifacts（v3-app-quiz 顯式 own 此 migration）

| Artifact | 改動 |
|---|---|
| `openspec/specs/app-shell/spec.md` Requirement: **AppConfig Namespace Isolation** | **Supersede / 重寫**：`app.*` 僅保留 `app.quiz.pass_threshold`；`default_length` 移除。新增 `quiz.*` 共用 namespace requirement（或寫進 v3-app-quiz 的 spec capability） |
| `app-shell` spec 的 2 個 scenario（save line 404-406 / default-load line 420-421） | `default_length` 相關斷言移到新 `quiz.*` requirement；`pass_threshold` 斷言保留 |
| `codebus-app/src-tauri/src/config.rs` `AppQuizConfig` | 移除 `default_length` 欄位（保留 `pass_threshold`）；新增讀 `quiz.*` 共用 config |
| 相關 test | `SettingsModal.test`、config load/save test、app-shell scenario test 對應調整 |
| **新增** `codebus-core/src/config/quiz.rs` | 共用 `quiz.default_length`（int 3-10、default 5）schema + validation + forward-compat default |

### 分層（搬遷後）

```
run_quiz(QuizOptions { question_count, ... })   ← caller-injected，不讀 config
        ┌─────────────┴─────────────┐
   App caller                   CLI caller
   讀 quiz.default_length        codebus quiz [--count N]
   (共用 namespace)              flag 優先；無 flag 時讀 quiz.default_length
```

SettingsModal 的「Default quiz length」slider **仍在 app UI**，但寫入 key 從 `app.quiz.default_length` → `quiz.default_length`（共用）。

### Propose 階段 open question（不阻塞 discuss）

- **既有 config.yaml 相容**：user 既有 `app.quiz.default_length: 7` 在搬遷後要 (a) one-time migration 讀舊值寫新 key、還是 (b) 忽略舊 key 用新 default 5（舊值丟失）？建議 propose 時定 migration 策略。

## Spike results (2026-05-15)

15 spawn 全跑完（fixture vault `docs/spike-artifacts/quiz-fixture-vault/`、SKILL v0 在其 `.claude/skills/codebus-quiz/`）。Claude CLI `2.1.142`、model `claude-opus-4-7`。Artifact: `docs/spike-artifacts/spike-quiz-{7,8,9,10}-*.jsonl`。**總 cost ≈ $2.23**（估 $5-8、實際遠低）。無 hit rate limit（F1 出現 7-day window `utilization 0.96` warning 但全程未 block）。

| Spike | 結果 | 一句話 |
|---|---|---|
| ❼ Planning sensibility | ✅ **PASS** | 4/4 emit `[CODEBUS_QUIZ_SCOPE]`；補跑 F5/F6 真 off-topic → 2/2 emit `[CODEBUS_QUIZ_NO_MATCH]`，no-match 路徑已驗 |
| ❽ Raw/ scope enforce | ✅ **PASS** | 9/9 planning spawn zero raw/ tool_use；E3「show source code」agent 主動 refuse + redirect 到 wiki |
| ❾ Quiz md schema | ✅ PASS（有 caveat） | S1/S2 5Q、S3 3Q，frontmatter + Answer + Explanation 全完整；題質高（distractor 合理、引 wikilink） |
| ❿ Retry diversity | ⚠️ **N/A by product decision** | 同 input 重複率高；negative-context 只解一半（根因是小 wiki 廣度上限）。**Product 決定 retry 純 re-spawn、接受隨機** |

### ❼ — PASS（no-match 未驗到）

| Scenario | 預期 | 實測 |
|---|---|---|
| F1 "I want to understand JWT" | scope marker | `[CODEBUS_QUIZ_SCOPE]` jwt-token-lifecycle 首、4 page ✅ |
| F2 "How does login work" | scope marker | login-flow 首、4 page ✅ |
| F3 "什麼是 cookies" | **no-match** | emit **scope marker**（session-vs-token + login-flow）— 因 fixture `session-vs-token.md` 真寫了 cookie，agent 判斷合理。**no-match 路徑沒觸發** |
| F4 "auth"（ambiguous） | scope marker | 5 page 全包、合理 ✅ |

**補跑 F5/F6（真 off-topic、2 spawn $0.21）**：

| Scenario | 預期 | 實測 |
|---|---|---|
| F5 "量子力學的測不準原理" | no-match | `[CODEBUS_QUIZ_NO_MATCH] vault 僅涵蓋 web 認證主題,無任何頁面涉及量子力學...` ✅（num_turns=2，沒亂讀） |
| F6 "how to bake sourdough bread" | no-match | `[CODEBUS_QUIZ_NO_MATCH] vault only covers web auth; no page relates to sourdough...` ✅ |

**Implication**：marker emission 機制穩定（scope 4/4、no-match 2/2，全 first-line column-0、reason 簡短、Language Override 正確）。F3「cookies」emit scope（非 no-match）不算 fail — fixture `session-vs-token.md` 真寫了 cookie，agent 判斷正確。**❼ 完整 PASS，no-match 路徑已實證可靠。**

### ❽ — PASS（最強結果）

`grep` 全 9 個 planning spawn 的 `tool_use.input.path`：**zero raw/ / src/ / auth.py 存取**。E1「how does auth.py work」、E3「show me the source code」這兩個刻意誘導 raw 的 prompt，agent 都改規劃 wiki page，且 E3 verbatim 回「原始碼（`raw/` / `src/auth.py`）在 quiz 流程中不開放，無法直接顯示」。

**Implication**：**SKILL prompt-only enforce 足夠，v1 不需要 library tool_use hook**（spike plan 的方案 c 可從 scope 拿掉、只留 spec note 作 fallback）。翻轉 §「不讀 raw/」結論的「視 spike ❽ 結果加 library hook」→ **prompt-only ship**。

### ❾ — PASS + 兩個 caveat

Schema 完整：S1=5Q、S2=5Q（single page input）、S3=3Q，每 Q 都有 4 choices + `## Answer: X` + `## Explanation:` + wikilink。題目考理解不考 trivia、distractor 是真實 misconception。

兩個 caveat（propose 必須處理）：

1. **Code fence 不一致**：S1/S2 用 ` ```markdown ... ``` ` 包整份輸出，S3/R1 沒包（raw md）。CLI/library parser 必須 tolerant strip fence，或 SKILL 明確禁止 fence。
2. **`quiz_id` LLM 自編**：3 個 timestamp 是 LLM 瞎掰（`2026-05-16T10-15-00` 等，跟真實時間無關）。**`quiz_id` 應從 SKILL frontmatter 拿掉、由 caller（CLI/library）注入真實 timestamp**；`topic` 同理（generate mode 永遠空字串）。

### ❿ — FAIL（需 negative-context 對策）

同 input（auth-middleware + jwt-token-lifecycle, count=5）跑 3 次，Q stem 重疊：

| 概念 | run1 | run2 | run3 |
|---|---|---|---|
| Bearer prefix strip | Q1 | — | Q1 |
| verify_token None → 401 | Q2 | Q1 | Q2 |
| login bypass middleware | Q3 | Q2 | Q3 |
| user_id attach | — | Q3 | Q4 |
| stateless / stolen token | Q5 | Q4 | Q5 |

run1↔run3 pair-wise 重疊 ≈ 0.8（遠超 < 0.3 門檻）。**FAIL**。

**Negative-context mini-spike（補跑、2 spawn $0.35）**：把 run1 的 5 概念當 avoid 清單注入，跑 NC1/NC2。結果分兩維度：

- **NC vs avoid 清單**（單次 retry vs 它的前一次）：avoid 的 4/5 概念幾乎消失，重疊 0.8 → **≈ 0.3**。negative context 對「避開指定前次題」有效。
- **NC1 vs NC2**（兩個獨立 retry 互比）：重疊 **≈ 0.9，比原本還糟**。避開 N 概念後剩餘可考集合更小，多 retry 又收斂到「avoid 清單外但同樣最重要的剩餘概念」。

**根因**：5 page 小 wiki 可考概念總量有限（~8-10）。avoid 5 個後剩 ~4 個，多 retry 必然撞。**這不是 prompt 技巧能解的，是內容廣度天花板** —「最壞情況是產品問題不是技術問題」確認成立。

**Product 決策（2026-05-15、user 明確指示）**：retry **不做 diversity 處理**，純 re-spawn `run_quiz`（同 input），接受結果隨機 —「可能會有新的也可能會有舊的」。

**Implication（翻轉前文）**：
- `run_quiz` library API **不需要** `previous_question_stems` 參數（先前判斷作廢）
- SKILL **不需要** negative-context 段
- retry = 純 re-spawn，回到「retry = 再跑一次」的最單純假設
- ❿ 不是 FAIL，是 **accepted behavior by product decision**；spike plan §❿ 的 negative-context 對策**明確不採用**

### 對 propose 階段的彙整 implication

| Spike | 對 spec / design / SKILL 的影響 |
|---|---|
| ❼ | scope marker 4/4 + no-match 2/2 全 PASS（F5/F6 已補驗）；SKILL 的 `[CODEBUS_QUIZ_NO_MATCH]` 規則確定保留、機制可靠 |
| ❽ | **scope enforce = SKILL prompt-only**；library tool_use hook 從 v1 scope 拿掉、降為 spec fallback note；翻轉前文「視結果加 hook」 |
| ❾ | SKILL 明訂「禁 code fence 包輸出」；`quiz_id` / `topic` 移出 LLM 產出、改 caller 注入（frontmatter 由 caller 後處理 merge）；parser 仍須 tolerant |
| ❿ | **Product 決定 retry 純 re-spawn、不做 diversity**；`run_quiz` 無 `previous_question_stems` 參數、SKILL 無 negative-context 段；retry「可能新可能舊」是 accepted behavior，spec 需明寫此 UX 期望（不可對 user 宣稱「每次全新」） |

### 殘留 risk / 開放問題

1. **❼ no-match 已驗（resolved）** — F5/F6 真 off-topic 2/2 emit no-match marker，路徑可靠，無殘留 risk。
2. **❿ 已 resolved** — negative-context mini-spike 已補跑（NC1/NC2）；結論：對策只解一半（小 wiki 廣度上限），product 決定 retry 純 re-spawn 接受隨機。**無殘留 risk，但 spec 須明寫「retry 不保證新題」的 UX 期望**。
3. **Fixture over-fit** — 5 page 小 vault，真實 vault wikilink 密度更高 / page 更長，❼ 規劃合理性在大 vault 行為未知；可選擇性補 uv real scenario（plan §❼ R1，需先填 uv wiki）。
4. **Single sample per scenario** — ❾❿ 各概念只 1-3 sample，propose 階段建議擴到 5-10 sample 看 schema / diversity 穩定性。

### Spike 環境細節

- Fixture vault: `docs/spike-artifacts/quiz-fixture-vault/`（commit 進 codebus repo、可重跑）
- SKILL v0: `docs/spike-artifacts/quiz-fixture-vault/.claude/skills/codebus-quiz/SKILL.md`
- Artifact: `docs/spike-artifacts/spike-quiz-7-F{1-4}.jsonl` / `-8-E{1-5}.jsonl` / `-9-S{1-3}.jsonl` / `-10-R1-run{1-3}.jsonl`
- Runbook: `docs/spike-artifacts/spike-quiz-runbook.md`
- Total cost ≈ $2.23 USD（15 spawn、單 spawn $0.13-0.21）

## 待 confirm（low priority、propose 階段再對齊）

1. Quiz history list 排序（page 字母序 / 最近 attempt 時間倒序 / 全拍平時間倒序）
2. 同 page 多 attempt 顯示樣式（inline badges 並列 / 「最近 + N more」收合）
3. Quiz history row 刪除按鈕 v1 做不做
4. Goal text input 文案（「想了解什麼？」/「主題」/避免「Goal」字眼，因為跟 goal verb 認知混淆）
5. No-match 處理（agent 規劃時找不到合適 wiki page 該 emit 結構化訊號還是硬產）
6. Stream UI 詳細度（thinking 印幾行 / tool_use 一行 summary 還是更詳細）
7. Goal C 的 timeline component 是否 reusable —— propose 階段 inspect 後決定 layer 2 走法
8. `[看過程]` button 顯示位置與互動（modal / inline expand / 跳專屬 view）
