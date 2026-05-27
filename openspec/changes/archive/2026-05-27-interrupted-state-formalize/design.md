## Context

本 change 是 Phase 6 v1.1 mock landing 第一塊，對應 `codebus-app/design-handoff/v1.1-mocks.html` §02c 與 `codebus-app/design-handoff/AUDIT.md` R7-2 / Phase 5.1 obsolete trailer / Phase 6 `interrupted-state-formalize` 三處規格來源。原 Phase 5.1 純 rename change 在 AUDIT 2137-2152 行已 obsolete，rename + 行為合併 + state machine 整理 + backend `interrupt_reason` 對接全部併入本 change。

### Pre-apply 校準（grep ground truth vs prompt vs mock 三方對齊）

實機 grep 結果揭露 **6 個** prompt 描述 / mock 規定 / 實機現況的差異，apply 階段以本段為準（per 累積教訓 project_phase_3a_blind_spots_cleanup_lessons + project_quiz_fullscreen_wizard_view_term_disambiguation）：

1. **Workspace.tsx routing 行號**：prompt 寫「約 line 555-582」，實機是 `codebus-app/src/components/workspace/Workspace.tsx` 555-585 行；既有 outcome switch 已是「cancelled|failed → RunDetailCancelled / interrupted → RunDetailInterrupted」三分流，不是 prompt 描述的「兩 component 都同時處理 cancelled/failed」。
2. **兩 component 內部差異**：99% duplicate，差別只有 4 處 testid（`run-detail-cancelled` vs `run-detail-interrupted`、`cancelled-badge` vs `interrupted-badge`、`cancelled-warning` vs `interrupted-warning`）+ 2 個 i18n key（`cancelledWarning` vs `interruptedWarning`）。其餘 100% 相同；連 StatusPill 都同時用 `status="interrupted"`（cancelled 顯 interrupted pill 是現況偏移）。
3. **`interrupted` outcome 性質**：「Virtual interrupted entries (no on-disk RunLog row) project into the same shape」── interrupted 是 GUI-side synthesized virtual value、backend 不寫 `outcome="interrupted"` 的 RunLog row。詳見 `openspec/specs/app-workspace/spec.md` 873 行 `Interrupted Run Detection` requirement。
4. **`interrupt_reason` 寫入位置**：prompt 寫「GoalRun state schema」，grep 結果 codebus-core 不存在「GoalRun」struct，真正 sink 在 `codebus-core/src/log/sink.rs` 的 `RunLog` struct（既有 `outcome: String` 欄位、不是 enum）。`interrupt_reason` 加在 `RunLog` 結構上、frontend `RunLogSummary` 在 `codebus-app/src/lib/ipc.ts` 對應 widen。
5. **3 sub-variant 分類**：prompt 寫「user-cancelled / agent-failed / system-interrupted」三 banner variant；mock §02c 1325/1329/1333 行寫的是 app-close / user-cancel / network-drop 三 interrupt_reason sub-variant（全在 Interrupted 殼下）、Failed 是另一視覺（mock 1284-1297 行明示 Failed=red、Interrupted=amber 兩個拉開）。**本 change 採 mock 版本為準**：Failed 是頂層 outcome banner（red 色語）、Cancelled+Interrupted 共用 amber 殼層並依 `interrupt_reason` 切 3 sub-variant 文案。
6. **Retry / Cluster 行為**：實機 Retry 是 NewGoalModal pre-fill（user 在 modal 內按 Run 才 spawn），與既有 spec L485「Retry SHALL NOT spawn a new goal directly」一致；本 change 維持不改。實機 partialTimeline 是 hard-code Read/Glob/Grep/Write/Edit count 三行字（reading/writing/other），未 reuse 5.3 ActivityCluster/clusterTimeline；本 change 維持不改。

### 同名詞 disambiguation（per project_quiz_fullscreen_wizard_view_term_disambiguation 教訓）

| 「state」維度 | 含意 | 在本 change 的角色 |
|---|---|---|
| **Outcome** | `RunLogSummary.outcome` 字串（running / succeeded / failed / cancelled / interrupted） | 既有值，不擴；本 change 只新增 `interrupt_reason` 旁敲側擊 |
| **Banner variant**（頂層） | UI banner 視覺色語：red（Failed）vs amber（Cancelled + Interrupted）| **新增** ── 對應 mock 1284-1297 視覺差表 |
| **Reason sub-variant**（amber 內細分） | 依 `interrupt_reason` enum 切的 banner 文案：app-close / user-cancel / network-drop / other | **新增** ── 對應 mock 1325/1329/1333 行 |
| **`interrupt_reason` field** | RunLog Rust struct 上的 `Option<InterruptReason>` 欄位 | **新增** ── Backend schema 擴張 |

三層各自獨立、apply 階段命名與斷言 SHALL 不混淆。

## Goals / Non-Goals

**Goals:**

- 把兩個 99% duplicate 的 `RunDetailCancelled` / `RunDetailInterrupted` component 合併為單一 `RunDetailInterrupted`，state machine 顯式化。
- Workspace.tsx outcome switch 由三分流（succeeded / cancelled|failed / interrupted）整併為「succeeded → RunDetailDone / running → RunDetailRunning / 其他終態 → RunDetailInterrupted」，後者 component 內部依 outcome + interrupt_reason 切 banner sub-variant。
- 對齊 mock §02c：Failed banner 視覺（red）與 Interrupted/Cancelled banner（amber）色語拉開；Interrupted/Cancelled 殼內依 `interrupt_reason` 切 4 sub-variant 文案（app-close / user-cancel / network-drop / other）。
- Backend `codebus-core/src/log/sink.rs` RunLog struct 新增 `interrupt_reason: Option<InterruptReason>` 欄位，serde Optional + backward compat（legacy jsonl row deserialize 為 None）。
- Frontend `RunLogSummary` 對應 widen `interrupt_reason?: InterruptReason`。
- i18n 在 en + zh 兩 bundle 新增 banner.failedTitle / failedSubtitle / interruptedTitle / interruptedSubtitle / reason.{appClose,userCancel,networkDrop,other} 等 key（既有 cancelledWarning / interruptedWarning / cancelledBadge / interruptedBadge / retryButton 不改名）。

**Non-Goals:**

- 不引入新的頂層 outcome 變體（RunOutcome 既有 5 值不擴）。
- 不改 Retry 行為（維持現行 NewGoalModal pre-fill seam、與 app-workspace spec L485 一致）。
- 不引入 5.3 ActivityCluster / clusterTimeline reuse（維持簡化 partialTimeline 三行 count）。
- 不改 RunDetailDone / RunDetailRunning 的視覺與行為。
- 不改 5.1 ChatWidget pulse、5.3 ActivityCluster 行為、Wiki / Quiz / Settings 的範圍。
- 不翻譯 `interrupt_reason` enum identifier（identifier 性質、kebab-case 字面值不譯）。

## Decisions

### Decision: 合併兩 component 為單一 RunDetailInterrupted（不另抽 Failed component）

**選擇**：把現有 `RunDetailCancelled.tsx` 內 `RunDetailCancelled` + `RunDetailInterrupted` 兩 export 合併為單一 `RunDetailInterrupted` 函式 component，檔案 rename 為 `RunDetailInterrupted.tsx`，test 檔同步 rename。

**理由**：實機兩 export 99% duplicate（差別只 4 個 testid + 2 個 i18n key），維護兩份等同邏輯是雜訊源，且已產生 StatusPill `status="interrupted"` 對 cancelled 也用的偏移（grep 結果證實）。Failed 也共用 layout 殼層（header / partial timeline / Retry footer / NewGoalModal），差別只在 banner 色語 + 文案，合進同一 component 由 outcome + interrupt_reason 切 sub-variant 比拆三 component 更少重複。

**Alternatives 考慮**：
- 純檔名 rename（不合併）：被 AUDIT 2141 行 obsolete 過（命名 collision、不解決 duplicate）。
- 把 Failed 拆獨立 component：layout 殼層相同，拆出去等於再造一份 duplicate。

### Decision: Workspace.tsx outcome switch 整併為兩分流

**選擇**：`codebus-app/src/components/workspace/Workspace.tsx` 555-585 行 switch 整併。`succeeded` → RunDetailDone、`running` 既有 path（`activeRunId === selectedRunId` 早判）→ RunDetailRunning、其餘三個終態 outcome（cancelled / failed / interrupted）→ RunDetailInterrupted，並把 outcome 與 `summary.interrupt_reason` 當 prop 傳入。

**理由**：合併後 component 自己 own state machine，Workspace.tsx 只負責「succeeded vs 非 succeeded」一刀分。減少 routing 層案例數，且把 banner sub-variant 邏輯收斂到 component 內部（mock §02c 1163-1164 行明示「v1.1 第三個正式 goal-detail state — Running / Done / Interrupted」三分法）。

**Alternatives 考慮**：保留 Workspace.tsx 三分流，component 內只切文案 ── 與「state machine 顯式」目標背離；apply 階段若 cancelled / failed / interrupted 視覺要再分歧仍要回 Workspace.tsx 改，內聚性差。

### Decision: Component 內部 state machine 顯式 switch

**選擇**：`RunDetailInterrupted` 內部用一段顯式 `switch (outcome)` 決定 banner color tier，再依 `interruptReason` 用第二段 switch 決定 sub-variant 文案；不採巢狀三元 / 條件 className 拼接。

```
outcome === "failed"
  → banner tier = "red"
  → title = banner.failedTitle  /  subtitle = banner.failedSubtitle

outcome === "cancelled" || outcome === "interrupted"
  → banner tier = "amber"
  → title = banner.interruptedTitle
  → subtitle = banner.reason.{appClose | userCancel | networkDrop | other}
                 或 banner.interruptedSubtitle（reason undefined fallback）
```

**理由**：與 prompt 「不要 hide-and-seek conditional rendering、要明顯 switch」對齊；分支對 reviewer 直觀，testid + banner color class 可由 tier 推導。

**Alternatives 考慮**：用 map object 索引 ── 對「cancelled / interrupted 共用、failed 拆開」這種非對稱分組可讀性不如 switch；map 缺 exhaustiveness 檢查。

### Decision: interrupt_reason 加在 RunLog struct、kebab-case enum + Optional + serde skip_if_none

**選擇**：在 `codebus-core/src/log/sink.rs` 新增 `InterruptReason` enum 與 `RunLog.interrupt_reason` 欄位。

```rust
#[serde(rename_all = "kebab-case")]
pub enum InterruptReason {
    AppClose,
    UserCancel,
    NetworkDrop,
    Other(String),
}

pub struct RunLog {
    // ...existing fields...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interrupt_reason: Option<InterruptReason>,
}
```

序列化為 "interrupt_reason":"app-close" / "interrupt_reason":"user-cancel" / "interrupt_reason":"network-drop" 字面字串，對齊 mock 1325/1329/1333 行；Other(String) variant 走 untagged 形式（`{"other":"..."}`）。

**理由**：
- `Option<InterruptReason>` + serde default + skip_serializing_if Option::is_none 維持向後相容，legacy jsonl row（無 `interrupt_reason` key）deserialize 為 None、不 crash；與既有 `session_id` 欄位同模式一致。
- `rename_all = "kebab-case"` 對齊 mock 字面值，且避開既有 5 處 internally-tagged enum collision（grep 結果：parser.rs 第 59 行、log/factory.rs 第 20 行、verb/event.rs 第 34/45/134 行使用 tag = "kind" 或 "sink" 或 content = "data"）；InterruptReason 不採 internally-tagged 形式，kebab-string variant 不會 collision。
- Other(String) variant 保留未來擴張空間（mock 未列舉的中斷類型如 agent-crash / runtime-panic 走 Other 不破 schema）。

**Alternatives 考慮**：
- enum 寫死 3 variant 不留 Other：未來新增中斷分類要動 schema migration。
- 不加 enum、直接用 `interrupt_reason: Option<String>`：失去 type-level exhaustiveness 檢查、frontend switch 寫死字串容易 typo。
- 加在 RunLogSummary projection 而非 RunLog 本體：Summary 是 frontend 視角投影，欄位來源仍要從 backend write 進 jsonl；本體加才是 source of truth。

### Decision: Frontend RunLogSummary widen + InterruptReason 型別匯出

**選擇**：`codebus-app/src/lib/ipc.ts` `RunLogSummary` interface 加 `interrupt_reason?: InterruptReason`；新增 type alias `export type InterruptReason = "app-close" | "user-cancel" | "network-drop" | { other: string }`。

**理由**：跟既有 `RunOutcome` 的 closed-set union 模式一致；component switch 用 union narrowing 即可。

**Alternatives 考慮**：把 InterruptReason 寫成 enum module ── ipc.ts 全檔風格是 type alias + interface，引 enum 不一致。

### Decision: Retry 行為與 partialTimeline 維持現況

**選擇**：本 change 不改 `Retry with same goal` 按鈕的「開 NewGoalModal pre-fill」行為（與 app-workspace spec L485 + 現有 Retry pre-fills modal without spawning scenario 一致）；不改 partialTimeline 簡化 reading/writing/other 三行 count 的呈現。

**理由**：
- prompt 描述「點 Retry → 用 same goal + same vault context 建立新 GoalRun」與既有 spec 衝突；衝突 by 現行 spec 為準（per feedback_quote_user_words_before_propose 教訓的反例提醒）。如要改成「直接 spawn」需另開 change，因會影響「user 必須 confirm」UX 契約。
- 引入 5.3 ActivityCluster reuse 會擴大本 change > 1 天工時上限，且 mock §02c 本身沒指明 cluster 呈現。

**Alternatives 考慮**：本 change 順手帶 cluster reuse ── 拒絕，違反 prompt「工時上限半天-1 天」+ AUDIT「不重造、reuse 5.3」也只是後續方向、非本 change scope。

## Implementation Contract

### Behavior（end-user 觀察）

- 跑 goal 後 cancel：detail view header 顯示 `⏹ Cancelled` badge、banner 為 amber 色語、文案走 `banner.reason.userCancel`（cancel 來自 user 主動）；點 `Retry with same goal` 開 NewGoalModal 並預填 goal text。
- Goal 跑到 agent 非零 exit：detail view header 顯示 `⚠ Failed`（新 badge）或對應 i18n value、banner 為 **red 色語**、文案走 `banner.failedTitle` + `banner.failedSubtitle`；點 Retry 行為同上。
- App 在 goal 跑到一半被關閉後重啟：Goals overview 列出 virtual interrupted entry（既有 Interrupted Run Detection requirement 不變）、detail view header `⚠ Interrupted`、banner amber、文案走 `banner.reason.appClose`；點 Retry 行為同上。
- 切 locale en ↔ zh：banner title / subtitle / reason 4 sub-variant + Failed banner 文案皆翻；`interrupt_reason` enum 字面值（app-close / user-cancel / network-drop）為 schema identifier、bundle value 仍填英文字面、不譯。

### Interface / Data Shape

**Backend RunLog struct**（`codebus-core/src/log/sink.rs`）：
- 新增欄位：`interrupt_reason: Option<InterruptReason>`，serde attr `#[serde(default, skip_serializing_if = "Option::is_none")]`。
- 新增 enum：`pub enum InterruptReason { AppClose, UserCancel, NetworkDrop, Other(String) }`，serde attr `#[serde(rename_all = "kebab-case")]`。
- 序列化字面："interrupt_reason":"app-close" / "user-cancel" / "network-drop"；Other(String) 走 untagged `{"other":"..."}`。

**Frontend type**（`codebus-app/src/lib/ipc.ts`）：
- `RunLogSummary` interface 加 `interrupt_reason?: InterruptReason` 欄位。
- 新增 type alias `export type InterruptReason = "app-close" | "user-cancel" | "network-drop" | { other: string }`。

**Component 簽名**（`codebus-app/src/components/workspace/RunDetailInterrupted.tsx`）：
- Props：`{ detail: RunDetail; vaultPath: string; onBack: () => void; onRetrySpawned?: (runId: string) => void }`（不變，從既有兩 component 沿用）。
- Component 內部從 `detail.summary.outcome` 與 `detail.summary.interrupt_reason` 讀 state machine 輸入，不再經 prop 注入 outcome。

**i18n key（新增、en + zh 兩 bundle）**：
- `workspace.runDetail.banner.failedTitle` / `failedSubtitle`
- `workspace.runDetail.banner.interruptedTitle` / `interruptedSubtitle`
- `workspace.runDetail.banner.reason.appClose` / `userCancel` / `networkDrop` / `other`

**i18n key（既有、不改名）**：
- `workspace.runDetail.cancelledBadge` / `cancelledWarning` / `interruptedBadge` / `interruptedWarning`：本 change 過渡期不刪，banner subtitle 取代主要文案來源；archive 後可清。
- `workspace.runDetail.retryButton`：完全不動。

### Failure Modes

- **Legacy jsonl row（無 interrupt_reason 欄位）reload**：deserialize 走 serde default → None；frontend `interrupt_reason` 為 undefined → banner subtitle fallback 走 `banner.interruptedSubtitle`（amber 殼層通用文案）；不 crash、不空白。
- **未知 interrupt_reason 字串**（frontend 看到 backend 寫了它不認識的 reason）：union narrowing 走 `{ other: string }` 分支 → banner subtitle 走 `banner.reason.other`；不 throw。
- **Backend 寫 Other(String) variant**：frontend deserialize 為 `{ other: "..." }`；UI 顯示通用「Other interrupt」文案（per i18n key `banner.reason.other`），原始字串不直接展示給 user（避免 leak schema-internal token）。

### Acceptance Criteria

1. `cargo test -p codebus-core` 綠，含：
   - RunLog serde round-trip test 涵蓋 `interrupt_reason: Some(AppClose)` / `Some(UserCancel)` / `Some(NetworkDrop)` / `Some(Other("agent-crash".into()))` / `None` 五 case。
   - Legacy jsonl row（無 interrupt_reason key）deserialize 為 `RunLog { interrupt_reason: None, .. }` 的 test。
2. `pnpm tsc` 綠（含新 InterruptReason type alias 與 RunLogSummary widen）。
3. `pnpm test` 綠，新 RunDetailInterrupted.test.tsx 覆蓋：
   - outcome=cancelled + 無 interrupt_reason → banner amber、文案 interruptedSubtitle。
   - outcome=cancelled + interrupt_reason="user-cancel" → banner amber、文案 reason.userCancel。
   - outcome=failed → banner red、文案 failedTitle/Subtitle。
   - outcome=interrupted + interrupt_reason="app-close" → banner amber、文案 reason.appClose。
   - outcome=interrupted + interrupt_reason="network-drop" → banner amber、文案 reason.networkDrop。
   - outcome=interrupted + 未知 reason（Other）→ banner amber、文案 reason.other。
   - 三 outcome 共用「Retry 按鈕點擊 → NewGoalModal open 且 textarea 預填 goal text、未發出 spawn IPC」既有契約（沿用 Retry pre-fills modal without spawning scenario 模式）。
4. **真實 CDP smoke**（zh + en locale，掃 project_cdp_smoke_webview2_pitfalls 5 雷後跑）：
   - 開 vault + 跑 goal + 點 Cancel → detail view 顯示 amber banner、文案對齊 reason.userCancel。
   - 跑 goal 並讓 agent 非零 exit（例：invalid path 或 mock 假 fail）→ detail view 顯示 red banner、文案對齊 failedTitle/Subtitle。
   - 跑 goal + kill codebus process 後重啟 → Goals overview 看到 virtual interrupted entry、進入 detail 看 amber banner、文案對齊 reason.appClose。
   - 切 locale：所有 banner title/subtitle/reason 翻譯、interrupt_reason enum 字面值不翻。
   - 截圖存 `codebus-app/scripts/.interrupted-smoke/`。
5. **Workspace.tsx routing 簡化驗證**：合併後 grep `RunDetailCancelled` 應為 0 hit（除了被刪檔的 git history），outcome switch case 數從 3 降為 ≤ 2（succeeded、其餘終態）。

### Scope Boundaries

**In scope**：

- `codebus-app/src/components/workspace/RunDetailCancelled.tsx` 兩 export 合併 + rename 為 RunDetailInterrupted.tsx、test 檔同步 rename。
- `codebus-app/src/components/workspace/Workspace.tsx` outcome switch 整併。
- `codebus-app/src/components/workspace/RunDetailInterrupted.tsx`（新）內部 state machine + 3 banner variant + 4 reason sub-variant。
- `codebus-app/src/lib/ipc.ts` RunLogSummary widen + InterruptReason type alias。
- `codebus-app/src/i18n/messages.ts` en + zh 兩 bundle 新增 banner.* + reason.* key。
- `codebus-core/src/log/sink.rs` RunLog struct 加 interrupt_reason 欄位 + InterruptReason enum 定義。
- `codebus-core/src/log/sink.rs` 既有測試擴增（serde round-trip 5 case + legacy compat）。
- **Backend verb cancel path 填 `Some(InterruptReason::UserCancel)`**：`verb::{goal,chat,fix,query}::run_*` 內 cancel-observe 寫 RunLog 時帶 `Some(UserCancel)`；`verb::quiz` 的 outcome=cancelled 分支同上；success / failure path 仍寫 `None`。**（apply 階段 user 選 Option 1 擴 scope）**
- **GUI synthesizer 填 `Some(InterruptReason::AppClose)`**：`codebus-app/src-tauri/src/ipc/goals.rs` `list_runs` 合成 virtual interrupted entry（orphan events jsonl）時填 AppClose；同檔 `RunLogSummary` IPC struct widen + `run_log_to_summary` projection 帶 `interrupt_reason`。**（apply 階段 user 選 Option 1 擴 scope）**
- `codebus-app/design-handoff/AUDIT.md` archive 階段標記 archived 2026-05-27 三處（R7-2 partial / Phase 5.1 obsolete trailer / Phase 6 條目）。

**Out of scope**：

- Retry 改成「直接 spawn 新 GoalRun」行為改動（保留現行 NewGoalModal pre-fill 模式）。
- 5.3 ActivityCluster / clusterTimeline 在 RunDetailInterrupted 內 reuse。
- RunDetailDone / RunDetailRunning 視覺改動。
- 新增頂層 RunOutcome variant。
- `NetworkDrop` variant 在 backend 自動填值（沒有對應的 detection seam；保留 enum variant 給未來 connection error path 用）。

## Risks / Trade-offs

- **[Risk] component 合併後 testid 變更可能 break 既有 Workspace.test.tsx / RunListItem.test.tsx 等斷言。** → Mitigation：grep `run-detail-cancelled` / `cancelled-badge` / `cancelled-warning` 三 testid 全 codebase usage，apply 階段一併更新；新 testid 一律 `run-detail-interrupted` / `interrupted-badge-{tier}` / `interrupted-banner-{reason}` 統一命名。
- **[Risk] backend 加 interrupt_reason 欄位但不在 cancel / fail / app-close path 自動填值 → GUI 永遠看 None / fallback subtitle。** → Mitigation：本 change Implementation Contract 明示「填值是 out of scope」，UI fallback 必有 graceful 文案；後續改 verb cancel path 寫 UserCancel、改 app-close detection 寫 AppClose 屬另外 change（或本 change apply 視工時餘裕一併處理）。
- **[Risk] mock §02c 1325/1329/1333 行所列 3 sub-variant app-close / user-cancel / network-drop 是否窮舉？** → Mitigation：Other(String) variant 保留擴張空間；frontend 文案 fallback 走 `reason.other`；未來新增 variant 不破 schema。
- **[Trade-off] 過渡期 i18n bundle 同時含舊 cancelledWarning / interruptedWarning key（未刪）與新 banner.* key**：增 bundle 字數，但避免 spec L490 / L495 既有 scenario 引用的 substring 失效。Archive 後可清。
- **[Risk] component 合併讓 Failed banner 第一次出現（之前 failed 走 cancelled 殼）視覺對齊偏移。** → Mitigation：Failed banner tier 走 red、與 Interrupted/Cancelled amber 明顯區隔；CDP smoke 涵蓋 failed case 驗收。
- **[Risk] solo dev 直接 main、無 PR review 緩衝；component rename + i18n + backend schema 同時動，git history 一次大改動。** → Mitigation：apply 階段循 tasks.md 順序 commit；test/build 綠才繼續下一段；archive 前用 spectra-drift 比對 spec 與實機是否吻合。
