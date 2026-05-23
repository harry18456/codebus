# T1 Spike：settings 缺 chat verb 的 model/effort 設定

**Date:** 2026-05-22
**Task:** loop T1（只讀探勘）
**背景:** [settings-chat-model backlog](2026-05-14-settings-chat-model-backlog.md)（2026-05-14，當時 status=parked）

---

## TL;DR — backlog 已部分過時

2026-05-14 backlog 推薦「方案 A（read-only hint）先做」。**方案 A 在 Claude 端其實已經做了**，只剩 Codex 端沒補。所以這項的剩餘工作比 backlog 寫的小：

- ✅ **Claude 已有 chat hint 列**：`EndpointSection.tsx:240` 有 `data-testid="endpoint-chat-row"`，顯示 `claudeCode.system.query.model/effort`（`:248-249`）——正是方案 A。
- ❌ **Codex 缺**：`CodexEndpointSection.tsx` 的 `VERBS=["goal","query","fix","verify"]`（`:29`）只 render 這四列，**沒有 chat hint 列**。
- 另外 backlog 預設 codex 還不存在（2026-05-14）；如今方案 B 的範圍比當時估的大。

---

## 現況（chat model/effort 怎麼解析）

兩 provider 都把 `Verb::Chat` fallback 到 `Verb::Query`，刻意設計（chat 是 read-only exploration）：
- Claude：`config/claude_code.rs:73`（system）/ `:94`（azure）→ `&...query`
- Codex：`config/codex.rs:98,107` → `&p.query`

`Verb::Verify` 則是**反例 / 也是方案 B 的現成範本**：它不 fallback，直接解析到專屬 `system.verify` / `azure.verify` 子塊（`claude_code.rs:78`，註解 `:42-48`）。verify 由 `verify-stage-independent-model` change 加入，整套「新增一個不 fallback 的 per-verb 設定」的改動軌跡可照抄。

## 方案 A 剩餘工作（補 Codex 的透明度）

**只動前端一個檔。** 在 `CodexEndpointSection.tsx` 仿 `EndpointSection.tsx:240-249` 加一列 read-only chat hint，顯示 codex 的 `query.model/effort`（因 codex 也是 Chat→query）。

- 檔案：`codebus-app/src/components/settings/CodexEndpointSection.tsx`(+ `CodexEndpointSection.test.tsx` 加一條 `endpoint-chat-row` 斷言)。
- 工程量：**輕（半天）**，純前端、零 schema、零 Rust、低風險。
- 收益：兩 provider 的 Settings 都明示「chat 沿用 query」，透明度問題收尾。

## 方案 B 剩餘工作（獨立 chat config）— 範圍已隨 codex 變大

照 `Verb::Verify` 的軌跡，但因 codex 已存在，需**兩 provider 都加 chat 子塊**：

**Rust（core）**
1. `config/endpoint.rs`：`SystemProfile` / `AzureProfile` 加 `chat: VerbConfig`（serde default）。
2. `config/claude_code.rs:73,94`：`Verb::Chat => &...chat`（取消 fallback）。
3. `config/codex.rs`：`CodexSystemProfile` / `CodexAzureProfile` 加 `chat`，`resolve` 的 `:98,107` 把 Chat 從 query 改 chat。
4. core 相關測試（resolve fallback 斷言要改）。

**前端（app）**
5. `store/settings.ts`：四處 normalizer（`:244,262,272,322`）+ `SYSTEM_PROFILE_DEFAULTS`（`:518` 區）+ codex 對應預設（`:410` 區）加 `chat`。
6. `EndpointSection.tsx` / `CodexEndpointSection.tsx`：把 chat 從 read-only hint 改成 `VERBS` 內的可編輯列（`VERBS` 加 `"chat"`，移除/取代現有 hint 列）。
7. `lib/ipc.ts`：`SystemProfile` / `AzureProfile`（+ codex interface）加 `chat` 欄位。
8. `lib/codex-validation.ts`(+test)：驗證涵蓋 `chat` 欄位（仿 verify）。
9. 前端測試：`EndpointSection.test.tsx` / `CodexEndpointSection.test.tsx` / `settings` 相關。

- 工程量：**中（1-2 個半天）**，跨 Rust+TS 但每步都有 verify 的前例可循、低不確定性。

## 建議

- **方案 A 剩餘（補 codex chat hint）值得順手做**：半天、單檔、收尾透明度。
- **方案 B** 維持 backlog 原判斷——等使用者真的反映 chat 需要與 query 不同的 model（PE1/PE2 的 codex 輸出議題可能間接催生：若結論是 chat 在 codex 需要更強 model，就會需要 B）才動。届時照 `Verb::Verify` 的軌跡實作。

## Out of scope
- 是否該給 chat 不同於 query 的「預設」model（屬產品決策，非本 spike）。

## 待 harry
此項要不要動？若只想收透明度尾巴 → 批准方案 A 補 codex（需解除「只讀」邊界，另起實作任務）。若要 chat 可獨立調 model → 方案 B。
