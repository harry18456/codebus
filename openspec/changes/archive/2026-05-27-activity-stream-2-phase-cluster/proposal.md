## Why

設計 v1.5 + AUDIT W4 / X1 系列鎖定「Activity stream 應呈現 2-phase 語意 cluster（READING CODEBASE / WRITING WIKI）」與「Shell tool row 必須附語意 kind 才能判定歸屬 cluster + 顯示乾淨 inner command」。目前 `ActivityStreamItem` 一條 event 一 row、無 cluster 摺疊、無 phase 分組、Codex provider 的 `powershell.exe -Command "<cmd>"` wrapper 還會吃掉 60+ 字截斷導致 actual command 被切。Backend `StreamEvent::ToolUse` 也沒有 shell 語意分類欄位，frontend 無從歸 cluster。

本 change 一次落地 backend schema（ToolUse 加 `tool_kind` 分類欄位 — 注意：不是 `kind`，因 `StreamEvent` enum 的 serde tag 已佔用 `kind`）+ codebus-goal skill emit 規範補充 + frontend 2-phase cluster rendering + mono ASCII icon prefix，把 W4 / X1 兩家 inter-locked 議題收尾。

## What Changes

- **Backend schema**：`codebus-core/src/stream/parser.rs` 的 `StreamEvent::ToolUse` 變體加 `tool_kind: Option<ToolKind>` 欄位；新增 `ToolKind` enum，5 個 variant：`Read`、`Inspect`、`Mutation`、`OtherRead`、`OtherWrite`（per AUDIT X1 § "對齊點 4 決議"）；缺欄位 fallback 為 `None`、frontend 視同 legacy 走 inspect-safe default
- **Parser**：`codebus-core/src/stream/parser.rs` 與 `codebus-core/src/stream/codex_parser.rs` 反序列化 `tool_kind`，缺欄位安靜 forward（不 panic、不 Err）
- **Skill emit 規範**：`.codebus/.claude/skills/codebus-goal/SKILL.md` 補「emit Bash/Shell tool 必須帶 `tool_kind`」段；intent 拿不到時 default `"inspect"`（per AUDIT X1 安全 fallback）；codebus-quiz **不在本 change scope**（prod 不存在、只在 spike-artifacts）
- **Frontend type widen**：`codebus-app/src/lib/ipc.ts` 的 `StreamEvent` tool_use variant 加 `tool_kind?: ToolKind` 型別；新增 `ToolKind` union type
- **2-phase cluster rendering（frontend）**：`codebus-app/src/components/workspace/ActivityStreamItem.tsx` 與 `codebus-app/src/lib/streamEventSummary.ts` 共同支援「連續 same-phase tool_use event 摺疊成一個 cluster row」：
  - **READING CODEBASE cluster**：`Read` / `Glob` / `Grep` / `Shell tool_kind=read` / `Shell tool_kind=inspect`
  - **WRITING WIKI cluster**：`Write` / `Edit` / `Shell tool_kind=mutation`
  - `other-read` 歸 READING、`other-write` 歸 WRITING（per AUDIT 不開新欄位）
  - cluster 計數 **不算** thought block（per AUDIT W4 § 5 條細節 #5）
  - banner / thought 中斷時 cluster 收尾（per AUDIT W4 § 細節 #1 brand emoji 不收進 cluster）
  - cluster 可重複出現（thought→reading→thought→reading→writing 是合法序列）
  - **collapsible default**：goal 跑中 cluster 預設 open、跑完預設 closed（per AUDIT W4 § 細節 #4）
- **Icon prefix（frontend）**：mono ASCII icon、**不是 emoji**（per AUDIT W4 § "Icon 用 mono ASCII 不用 emoji"）：`Read` → `📄` / `Glob` → `🗂` / `Grep` → `🔍` / Shell.read → `$_` / Shell.inspect → `$?` / Write+Edit → `✎` / Shell.mutation → `$!`；icon 走 `streamEventSummary.ts` 統一吐、`RunListItem` 5.2 stream tail 自動 inherit（不需動 RunListItem 本檔）
- **Codex shell wrapper 抽取**：`extractInnerCommand` 已在 5.2 helper 抽好（X1 修法 1）、本 change 不重做、只確認 cluster icon 與已抽 inner command 並用
- **i18n**：`codebus-app/src/i18n/messages.ts`（單一 TS module、不是 JSON）加 cluster 相關 key：
  - `workspace.activity.cluster.reading.heading`（en: `Reading codebase`、zh: `讀檔案`）
  - `workspace.activity.cluster.writing.heading`（en: `Writing wiki`、zh: `寫 wiki`）
  - `workspace.activity.cluster.expand` / `workspace.activity.cluster.collapse`（aria-label）
  - `workspace.activity.cluster.summary.reading`（done collapsed：`12 reads · 195 shell · 6.2s` / `讀檔 12 次 · shell 195 次 · 6.2 秒`）
  - `workspace.activity.cluster.summary.writing`（done collapsed：`3 new · 2 updated · 4.5s` / `新增 3 · 更新 2 · 4.5 秒`）
- **Backward compat**：legacy 無 `tool_kind` 的 ToolUse event 視同 `Inspect`（safe default），cluster 仍能歸屬、不 break pre-W4 archived stream session

## Non-Goals

- 不動 W5 live tail（spinning circle + amber narration + blinking caret）— 與 W4 cluster 是兩個 issue、留下個 change
- 不動 W6 / D3 / X2 `CollapsibleStreamLog` shared component（02a/02b/02c 三態共用 raw stream log card）— 留 X2 專屬 change
- 不動 `RunListItem` 本檔（5.2 已把 stream tail wire 進 `summarizeVerbEvent`，本 change 改 helper 後自動 inherit）
- 不動 5.1 ChatWidget pulse dot 同源 state（reuse、不重做）
- 不動 codebus-quiz skill（prod 不存在、grep 校準確認、scope 收斂到 codebus-goal only）
- 不做 v1.1 spec 落地內容（02c Interrupted layout / ChatWidget 3 modes / Wiki page reader — Phase 6 範圍）
- 不採 hard cutover（schema field optional + frontend Inspect fallback）

## Capabilities

### New Capabilities

（無）

### Modified Capabilities

- `agent-stream-rendering`：`StreamEvent::ToolUse` 加 `tool_kind` optional 欄位、新 `ToolKind` enum、parser 反序列化規則、legacy event fallback 規則
- `app-workspace`：Activity stream 從「一 event 一 row」改成 2-phase cluster rendering、mono ASCII icon prefix、cluster collapsible default 規則
- `skill-bundles`：codebus-goal skill emit Shell tool 必須帶 `tool_kind` 的規範條目（codebus-quiz 不在 scope）

## Impact

- Affected specs：`agent-stream-rendering` + `app-workspace` + `skill-bundles`
- Affected code：
  - Modified：
    - codebus-core/src/stream/parser.rs
    - codebus-core/src/stream/codex_parser.rs
    - codebus-core/src/render/stream_event.rs（terminal renderer 也吃 schema 變動）
    - codebus-app/src/lib/ipc.ts
    - codebus-app/src/lib/streamEventSummary.ts
    - codebus-app/src/components/workspace/ActivityStreamItem.tsx
    - codebus-app/src/components/workspace/ActivityStreamItem.test.tsx
    - codebus-app/src/lib/streamEventSummary.test.ts
    - codebus-app/src/i18n/messages.ts
    - .codebus/.claude/skills/codebus-goal/SKILL.md
  - New：
    - codebus-app/src/components/workspace/ActivityCluster.tsx（cluster wrapper component、收 children + heading + collapsible state）
    - codebus-app/src/components/workspace/ActivityCluster.test.tsx
    - codebus-app/src/lib/clusterTimeline.ts（純函式：projected `TimelineItem[]` → `ClusterItem[]` 含 phase 歸屬）
    - codebus-app/src/lib/clusterTimeline.test.ts
  - Removed：（無）
