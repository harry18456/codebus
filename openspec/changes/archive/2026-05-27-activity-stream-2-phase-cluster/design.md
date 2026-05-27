## Context

Phase 5.3 是 AUDIT W4（Activity stream 2-phase cluster）+ X1（Shell tool 加 kind enum + Codex wrapper 抽取）兩個 inter-locked design issue 的合併實作。Phase 5.1（chatwidget-pulse-and-cancel-move）與 5.2（goals-running-row-stream-tail）已交付：

- 5.1 chat pulse dot 同源 state（active run）已 wire
- 5.2 把 `summarizeToolInput` / `bannerLabel` / `extractInnerCommand` / `writeEditPath` 抽到 `codebus-app/src/lib/streamEventSummary.ts` 並新增 `summarizeVerbEvent` facade、`RunListItem` 改吃 facade

本 change 動 helper（新增 cluster 邏輯 + icon prefix）+ backend schema（ToolUse 加 `tool_kind`）+ codebus-goal SKILL.md emit 規範、不重做 5.1 / 5.2 已落地的部分。

### Pre-apply 校準（grep 過 ground truth 後實機 vs brief 假設差異）

Brief 與 AUDIT 寫的描述跟實機 codebase 有以下需要 design 階段先校準的差異。Apply 第一步要依此調整 task 細節，不要照 brief 字面跑：

1. **Backend schema 不在 brief 寫的位置**
   - Brief 寫「codebus-core/src/render/stream_event.rs（actual emit point）」是錯的；此檔是 **terminal renderer**（消費 StreamEvent 印 CLI 字串）
   - 實際 schema 在 `codebus-core/src/stream/parser.rs:43` 的 `pub enum StreamEvent`
   - Apply 必須改 parser.rs 的 enum 定義、再回頭讓 renderer 跟著吃新欄位
2. **沒有 ShellEvent variant、shell command 是 ToolUse**
   - Brief 寫「ShellEvent variant 加 kind field」是錯的
   - 實際只有 `StreamEvent::ToolUse { name, input }`、shell 命令 name 是 "Bash" / "Shell" / Codex 端可能是其他
   - 新欄位 SHALL 加在 ToolUse variant 上（不只 shell；Read/Glob/Grep 也用同欄位歸 cluster）
3. **欄位名稱不能叫 `kind`、會撞 serde tag**
   - StreamEvent enum 已用 `#[serde(tag = "kind", rename_all = "snake_case")]` 把 kind 當 variant 識別符
   - 新分類欄位 SHALL 命名 `tool_kind`（snake_case、avoid collision、語意 explicit）
   - Frontend StreamEvent 也用 kind 做 variant tag（codebus-app/src/lib/ipc.ts L617）、同樣命名 `tool_kind`
4. **codebus-quiz prod 不存在**
   - Grep 結果：codebus-quiz SKILL.md 只在 `docs/spike-artifacts/quiz-fixture-vault/.claude/skills/`、prod `.codebus/.claude/skills/` 沒有
   - Scope SHALL 收斂到 codebus-goal only、不動 codebus-quiz
5. **5.2 helper 路徑 codebus-app/src/lib/streamEventSummary.ts、不是 activityStreamHelpers.ts**
   - 已導出：writeEditPath / extractInnerCommand / summarizeToolInput / bannerLabel / summarizeVerbEvent
   - 本 change SHALL 在同檔加 cluster 邏輯（或拆 clusterTimeline.ts 純函式檔、視 cohesion 決定）
6. **AUDIT vs Brief icon 風格衝突**
   - Brief 寫「emoji prefix（📖 / 🔍 / ✏️ / 🛠️）」
   - AUDIT W4 § "Icon 用 mono ASCII 不用 emoji" 明確規定 mono ASCII icon（📄 / 🗂 / 🔍 / $\_ / $? / ✎ / $!）— design v1.5 spec lock
   - 衝突取 **AUDIT 為準**（design v1.5 spec lock > brief 簡化描述）；apply 階段不要照 brief 的 emoji
7. **AUDIT vs Brief cluster 語意不同**
   - Brief 寫「連續同 kind 的 tool_use event 摺疊」
   - AUDIT W4 明確是「2-phase semantic split」：READING CODEBASE / WRITING WIKI 兩個 phase cluster；Read+Glob+Grep+Shell.read+Shell.inspect 都歸 READING（不是各自一 cluster）
   - 衝突取 **AUDIT 為準**
8. **Cluster collapsible default 規則**
   - Brief 寫「click cluster row 展開、預設不 cluster；單一不 cluster」
   - AUDIT W4 § 細節 #4 明確：**open during run / closed when complete**（不是 click 才展開）
   - 衝突取 **AUDIT 為準**：goal 跑中（status=running）cluster 預設展開、跑完（status=done/interrupted/failed）預設收起
9. **i18n cluster heading copy 要中英文化**
   - AUDIT § 02b 對齊點 5 明確「中文化」：done collapsed summary `讀檔 12 次 · shell 195 次 · 6.2 秒` / `新增 3 · 更新 2 · 4.5 秒`
   - en bundle 也要對應 key
10. **Cluster count 不算 thought block**
    - AUDIT W4 § 細節 #5 規定 cluster count 不算 thought
    - 是 clusterTimeline.ts 的邏輯點、specs 也要寫進 scenario

## Goals / Non-Goals

**Goals:**

- Backend StreamEvent::ToolUse 加 `tool_kind: Option<ToolKind>` 欄位，新增 5-variant enum
- Parser（claude + codex）解析 `tool_kind`、缺欄位 graceful fallback None
- Frontend StreamEvent.tool_use 同樣加 optional `tool_kind`
- 純函式 clusterTimeline.ts 把 TimelineItem[] 摺成 ClusterItem[]，含 phase 歸屬 + thought / banner 中斷 + cluster 可重複
- ActivityCluster.tsx 新 component：cluster heading + collapsible default 規則 + mono ASCII icon prefix + count（不含 thought）
- ActivityStreamItem.tsx 不再直接接 timeline，改吃 ClusterItem 或 cluster-aware 結構（5.2 RunListItem 透過 summarizeVerbEvent facade 不受影響）
- codebus-goal SKILL.md 補「emit Bash/Shell 必須帶 tool_kind」段、列五個 variant + 安全 fallback inspect
- i18n bundle 補新 key、en + zh 同時加（per Phase 4A G-copy-2 教訓 value-only 改 / key 不改名）
- Backward compat：legacy event 沒 tool_kind → frontend 視同 Inspect 歸 READING CODEBASE cluster；舊 stream session reload 不 crash

**Non-Goals:**

- W5 live tail（spinning circle + amber narration + blinking caret）— 下個 change
- W6 / D3 / X2 CollapsibleStreamLog shared component — X2 專屬 change
- 不動 RunListItem（5.2 已 wire summarizeVerbEvent facade、改 helper 自動 inherit）
- 不動 ChatWidget pulse dot 同源 state（5.1 已落地）
- 不動 codebus-quiz skill（prod 不存在）
- 不做 v1.1 spec 落地內容（02c Interrupted layout / ChatWidget 3 modes / Wiki page reader）

## Decisions

### 新欄位命名 tool_kind（不是 kind）

**選 `tool_kind`** 而非 `kind` / `category` / `class`。

- `kind` 撞 serde variant tag（rust StreamEvent `#[serde(tag = "kind")]` 已用、frontend StreamEvent union 也用 kind）；硬塞會 panic on deserialize
- `category` 太一般、未來再加非 tool 分類就模糊
- `class` 是 reserved 字（JS / TS）會干擾 IDE auto-complete
- `tool_kind` 語意明確（限縮在 tool 分類）、snake_case 跟 codebus 命名慣例一致（per feedback_spectra_propose_grep_naming_first）、無 collision

### 欄位 optional 而非 required（backward compat）

**選 `Option<ToolKind>` + frontend 預設 Inspect fallback**。

- Hard cutover（強制 required）：legacy archived stream session reload 全 crash、code 不對 emit 規範就斷掉、apply 階段易誤觸
- Optional + default：5 min 工、零 risk、自然 phase-out；solo dev 沒外部 user 也選這條（per feedback_engineer_best_not_easiest：選工程最正確解、不是省麻煩）

### Fallback default 走 Inspect 不走 Mutation

當 `tool_kind` 為 None / Codex skill 還沒帶 → frontend SHALL 視同 Inspect、歸 READING CODEBASE cluster。

- 取 AUDIT X1 §「拿不到 intent 時 default `inspect`（最安全 unknown——不歸到 mutation cluster，避免「咦它寫東西了？」誤覺）」

### ToolKind 5 個 variant 而非 3 個

`Read / Inspect / Mutation / OtherRead / OtherWrite`

`OtherRead` / `OtherWrite` 是「未來新 tool（Task / WebFetch / ...）read-like 或 write-like default」（per AUDIT X1 對齊點 4：不另開 `cluster` 欄位、用同 enum 擴展）。

### ActivityCluster 抽 new component、不塞進 ActivityStreamItem.tsx

- 5.2 helper 抽出時已示範這個 seam
- cluster 是 wrapper 邏輯（heading + collapsible + child events）、item 是 leaf 邏輯（單條 row）；混在一檔違反 SRP
- 純函式 clusterTimeline.ts 把 TimelineItem[] projection 成 ClusterItem[]、component 只負責 render
- 測試也分得開（純函式 unit test + component render test）

### Cluster collapsible default 跟 run status 連動

- running → cluster `open`（user 想看 LLM 現在在幹嘛）
- done / interrupted / failed → cluster `closed`（goal 已結束、user 預設只看 summary、要展才看細節）
- per AUDIT W4 § 細節 #4 lock

### 不收進 cluster：banner / thought

- banner（🚌 / 🎯 / 🚏 / 🎉）是敘事層、跨 cluster 流動、flat 排（per AUDIT W4 § 細節 #1）
- thought block 也 flat 排、不收 cluster、但 thought 出現會中斷上一個 cluster（cluster 收尾）
- cluster 之間隔 thought / banner、re-open cluster 是新 cluster 不是延續

### 中文化 done collapsed summary

- en：Reading codebase · 12 reads · 195 shell · 6.2s / Writing wiki · 3 new · 2 updated · 4.5s
- zh：讀檔 12 次 · shell 195 次 · 6.2 秒 / 新增 3 · 更新 2 · 4.5 秒
- per AUDIT 對齊點 5 決議

## Implementation Contract

### Behavior

跑 goal 後 02a RunDetailRunning 顯示的 Activity stream SHALL：

1. 連續 tool_use event（含 Read/Glob/Grep/Write/Edit/Bash 等）按 phase 歸屬合併為 ActivityCluster row
2. READING CODEBASE 與 WRITING WIKI 兩個 phase 各有 mono ASCII icon prefix（📄/🗂/🔍/$\_/$? 與 ✎/$!）
3. banner（🚌 start / 🎯 goal / 🚏 commit_done / 🎉 done）與 thought block 不收進 cluster、flat 顯示
4. cluster 在 running 狀態預設展開（child events 直出）、在 done/interrupted/failed 狀態預設收起（只顯示 heading + summary）
5. cluster 可重複出現（thought → reading → thought → reading → writing 是合法序列）
6. cluster count 只算 tool_use row、不算 thought
7. user 點 cluster heading 可切換 open / closed
8. legacy event（tool_kind 缺）視同 Inspect 歸 READING CODEBASE cluster、無 crash
9. Codex provider 的 powershell wrapper 顯示乾淨 inner command（5.2 extractInnerCommand 行為 unchanged）

### Interface / data shape

#### Backend Rust schema

```rust
// codebus-core/src/stream/parser.rs
pub enum StreamEvent {
    Thought { text: String },
    ToolUse {
        name: String,
        input: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_kind: Option<ToolKind>,
    },
    ToolResult { output: String, is_error: bool },
    Usage(TokenUsage),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Read,
    Inspect,
    Mutation,
    OtherRead,
    OtherWrite,
}
```

#### Frontend TS type

```ts
// codebus-app/src/lib/ipc.ts
export type ToolKind =
  | "read"
  | "inspect"
  | "mutation"
  | "other_read"
  | "other_write"

export type StreamEvent =
  | { kind: "thought"; text: string }
  | { kind: "tool_use"; name: string; input: unknown; tool_kind?: ToolKind }
  | { kind: "tool_result"; output: string; is_error: boolean }
  | ({ kind: "usage" } & TokenUsage)
```

#### Frontend cluster projection（純函式）

```ts
// codebus-app/src/lib/clusterTimeline.ts
export type ClusterPhase = "reading_codebase" | "writing_wiki"

export type ClusterItem =
  | { kind: "cluster"; phase: ClusterPhase; events: VerbEvent[]; count: number }
  | { kind: "event"; event: VerbEvent }
  | { kind: "thought_block"; text: string }

export function projectClusters(items: TimelineItem[]): ClusterItem[]
export function classifyToolPhase(event: VerbEvent): ClusterPhase | null
```

#### codebus-goal SKILL.md emit 規範新段

.codebus/.claude/skills/codebus-goal/SKILL.md SHALL 新增「Bash/Shell tool_kind emission」段，列：

- 必填欄位、5 個 enum value、各語意 + 範例（mirror AUDIT X1 表）
- 拿不到 intent 時 fallback inspect
- 範例 emit 指令

### Failure modes

- legacy event（無 tool_kind）→ frontend classifyToolPhase 回 "reading_codebase"（Inspect fallback）、不 crash、不 console.warn
- Codex 端傳了無效 tool_kind 值 → Rust serde reject 整個 event（保守、避免 silent data corruption；發 panic 不可接受、parser 已有「malformed → 0 events」習慣）
- Frontend 收到 unknown tool_kind 字串 → TS narrow 不過、運行期 fallback 走 reading_codebase（同 legacy 路徑）

### Acceptance criteria

1. `cargo test -p codebus-core` 綠（含 ToolUse serde round-trip + missing tool_kind graceful + ToolKind 5 variant 全綠）
2. `pnpm tsc` 綠（含 ToolKind union 在 type narrow / exhaustive check 都過）
3. `pnpm test` 綠（含 clusterTimeline.test.ts 純函式覆蓋 phase 歸屬 / thought 中斷 / banner 中斷 / cluster 可重複 / count 不算 thought / legacy fallback / ActivityCluster.test.tsx collapsible default 規則 / ActivityStreamItem.test.tsx 不 regress）
4. CDP smoke（zh + en locale、依 project_cdp_smoke_webview2_pitfalls 五雷預檢）：
   - 開 vault + 跑 codebus-goal → 02a RunDetailRunning 看到 cluster heading + mono ASCII icon prefix
   - 連續 read（Read/Glob/Grep）摺疊成一個 READING CODEBASE cluster
   - thought 中斷後新 READING CODEBASE cluster 重新開（cluster 可重複）
   - Write/Edit 摺疊成 WRITING WIKI cluster
   - 切 codex provider 跑 goal、Shell tool 帶 tool_kind 也正確歸 cluster
   - goal done 後 cluster 收起、顯示 summary（en + zh 字串）
   - click cluster heading 切換 open / closed
   - 截圖存 codebus-app/scripts/.activity-cluster-smoke/
5. Backward compat：找一個 pre-W4 archived run（events-\*.jsonl 無 tool_kind 欄位）reload、frontend 不 crash、event 以 legacy fallback 渲染 READING CODEBASE cluster

### Scope boundaries

**In scope:**

- StreamEvent::ToolUse 加 tool_kind 欄位 + ToolKind enum
- Claude + Codex parser 解析新欄位（graceful missing）
- Terminal renderer（codebus-core/src/render/stream_event.rs）跟著吃 enum 變動、不影響 CLI 行為
- Frontend StreamEvent union widen
- 新 clusterTimeline.ts 純函式 + ActivityCluster.tsx component
- ActivityStreamItem.tsx 改吃 cluster-aware 結構（但 leaf render 不變）
- streamEventSummary.ts 加 mono ASCII icon prefix 邏輯
- codebus-goal SKILL.md emit 規範新段
- messages.ts 加 cluster i18n key（en + zh）

**Out of scope:**

- W5 live tail（spinning + amber narration + caret）
- W6 / D3 / X2 CollapsibleStreamLog shared component
- codebus-quiz skill 任何改動
- ChatWidget 行為（5.1）
- RunListItem 本檔的渲染（5.2）
- v1.1 spec：02c Interrupted layout、ChatWidget 3 modes、Wiki page reader

## Risks / Trade-offs

| Risk | Mitigation |
|---|---|
| Backend serde tag 用 kind 名稱跟 brief 衝突、apply 階段誤改 | Decisions 已 lock tool_kind 命名；tasks 第一步 grep `#[serde(tag` 校準 |
| Cluster 邊界判定錯誤（漏中斷 / 過度合併） | clusterTimeline.test.ts 純函式測試覆蓋全部 fold scenario：thought 中斷 / banner 中斷 / 連續同 phase / cluster 可重複 / 跨 phase 不合併 |
| tool_kind 缺欄位的 legacy event 被當 mutation 誤歸 WRITING WIKI | Fallback 寫死 Inspect → reading_codebase；單元測試覆蓋 |
| Codex provider 已 emit 但 schema 不同（field 名 / 大小寫） | apply Task 1.1 校準 codex_parser.rs 實際 emit；若 codex 端尚無此欄位、design 允許 None fallback、不阻擋 ship |
| collapsible default 把 user 已開的 cluster 切回 closed | collapsible state 走 React local state（per cluster id）、不持久；status 變 done 時整個 re-mount cluster、預期重置 — 接受此 trade-off |
| mono ASCII icon 在窄 viewport / zh font fallback 不對齊 | icon 用 font-mono class、span aria-hidden="true" 包；視覺 QA 由 CDP smoke 截圖驗 |
| Cluster heading aria-expanded / aria-controls 漏接 a11y | ActivityCluster.tsx 強制 button + aria；test 用 screen.getByRole 確認 expanded |

## Migration Plan

無 schema migration（field 是 optional 加法、舊資料相容）。Deploy 後：

1. Backend ship → 舊 frontend 不受影響（多一個 unknown field、忽略）
2. Frontend ship → 新欄位 read-side 啟用、legacy event fallback 路徑生效
3. codebus-goal SKILL.md emit 規範生效 → 下次 spawn 開始帶 tool_kind
4. Pre-W4 archived run reload → fallback 路徑、視覺仍合理

Rollback：revert commit、無 schema rollback 需求。

## Open Questions

- ~~Codex provider 端是否已經 emit 對等欄位？~~ **Apply Task 1.2 校準結果（2026-05-27）**：Codex 端**完全沒有** `tool_kind` 概念。codex_parser.rs 把 `item.completed`.`command_execution` 一律 hard-code 成 `StreamEvent::ToolUse { name: "Shell", input: { command: ... } }`、無語意分類層、無 codex 端 SKILL.md（codex 不走 codebus skill 系統、由 codebus-core wrap）。本 change 不阻擋 ship：Codex 的所有 Shell call 永遠 `tool_kind: None` → frontend 走 Inspect fallback → 全進 READING CODEBASE cluster。**語意精度損失**（git commit / rm / mkdir 也歸 READING）為已知限制；未來若要還原語意，需在 codex_parser.rs 端加 heuristic 或要求 Codex CLI 端帶分類（後者要等 OpenAI 端、不可控）。標為 **deferred to follow-up change**。
- Done summary 字串「12 reads · 195 shell · 6.2s」的「195 shell」是「總 shell call 數」還是「shell.read + shell.inspect 數」？AUDIT 寫法是後者較合語意（READING cluster summary）但要 apply 階段對 design v1.5 確認；先採「READING 該 cluster 內 shell 命令數」
- ~~Glob / Grep tool 是否 emit 為 name: Glob / Grep~~ **Apply Task 1.3 校準結果（2026-05-27）**：兩 provider name set 差異：
  - **Claude 端**：tool name 由 Anthropic API forward 原始值，含 `Read` / `Glob` / `Grep` / `Bash` / `Write` / `Edit`，無 wrapper
  - **Codex 端**：所有 command 一律 `name: "Shell"`，無 `Read` / `Glob` / `Grep` 等細分（命令在 `input.command` 字串裡）
  - **Implication**：classifyToolPhase 必須處理兩條路徑：(a) `name in {Read, Glob, Grep}` → reading（Claude 走這條）；(b) `name == Shell` 或 `Bash` → 看 `tool_kind` 分類（Claude 帶；Codex 永 None → Inspect → reading）。clusterTimeline.ts doc comment 列此 alias 表給未來 reviewer。
