# PE2 設計：provider-specific prompt 策略 + codex parser 補強

**Date:** 2026-05-22
**Task:** loop PE2（只讀設計，不動實作；依賴 [PE1 診斷](2026-05-22-provider-prompt-diagnosis.md)）
**背景:** [provider-prompt-engineering backlog](2026-05-22-provider-prompt-engineering-backlog.md)

---

## 出發點（PE1 結論回顧）

兩類成因確定成立，分屬不同層、不同修法：

- **C1 指示失準**：skill bundle 對 codex 沿用 Claude 內容，寫死了 `--tools` / PreToolUse hook / `mcp_*` 等 codex 沒有的機制。
- **C2 parser 保真度**：codex parser 只映 3 種 event → 檔案編輯、錯誤、細工具呼叫不可見。

新確認的兩個邊界事實，縮小了設計範圍：

1. **`CLAUDE.md` / `AGENTS.md` 不用動。** 兩者都由 `schema/neutral.md`（`NEUTRAL_RULES`）產生（`vault/init.rs:329,474`），且有 `tests/schema_neutrality.rs` 強制其 provider-neutral。指示失準**只集中在 skill bundle 的 `stub_content`**（`skill_bundle/mod.rs:150-552`）。
2. **render 層只 match 4 個 `StreamEvent` variant**（`render/stream_event.rs:42-103`），且 Write/Edit 的特殊渲染是靠 `name == "Write" || "Edit"` 觸發（`:52`）。→ 只要 parser 把 codex 編輯映成 `ToolUse{name:"Edit"}`，**現有渲染直接適用，不必動 render / app**。

---

## C1 設計：skill 指示差異化

### 問題本質
不是「語氣」差，是 **skill 描述了 codex 不存在的機制**。最嚴重：quiz Mode B 的自我驗證契約寫「the PreToolUse hook permits `codebus quiz validate`」（`skill_bundle/mod.rs:418,479-486`）——codex 用 `--ignore-rules`，無 hook。

### 方案

| 方案 | 做法 | 優點 | 缺點 |
|---|---|---|---|
| **A. 機制無關化（建議 floor）** | 把 skill 裡「**如何**強制」的句子拿掉，只留「**是什麼**不變式」（如「This workflow is read-only; do not Write/Edit」），不點名 `--tools`/hook/`mcp_*` | 一次修好兩 provider；除掉謊言；零 provider 矩陣；維護最輕 | 失去對 Claude 的 defense-in-depth 機制說明（但那本來就重複，sandbox/`--tools` 在 binary 層已 gate） |
| **B. per-provider 參數化** | `stub_content(verb, Provider)`，機制句依 provider 分支（codex 描述 `-s sandbox`、無 hook 的現實） | 可給 codex 正確機制敘述 | 引入 provider 矩陣，`stub_content` 與雙寫呼叫點都要 thread `Provider`；維護成本翻倍 |
| C. 完整 per-provider skill 檔 | 各 provider 一套獨立模板 | 最大彈性 | 過早、最重，否決 |

**建議：A 為主**（機制無關化），**B 僅用於 codex 真正需要不同「指示」而非「機制描述」之處**——目前已知唯一這種點是 quiz Mode B 自我驗證（見下方未決問題）。理由：C1 的缺陷本質是「描述了不存在的機制」，把機制描述拿掉就同時修好兩邊，不需要 provider 分支去維護兩套謊言的「正確版」。

### 受影響檔案（A 方案）
- `codebus-core/src/skill_bundle/mod.rs`：改寫 `QUERY_WORKFLOW`(`:293`)、`CHAT_SKILL_CONTENT`(`:337`)、`QUIZ_SKILL_CONTENT`(`:418`)、`FIX_WORKFLOW`(`:526`) 裡點名 `--tools`/hook 的句子；同檔測試（`query_workflow_declares_read_only_invariant` 斷言 `"gated at the binary layer"` 等需同步改）。
- 工程量：**輕（約 1 個半天）**——主要是逐句改寫 + 修對應斷言；無跨 crate 影響。

---

## C2 設計：codex parser event 覆蓋擴充

### 方案

| 方案 | 做法 | 受影響面 |
|---|---|---|
| **1. 只擴 parser（建議）** | 在 `parse_codex_stream_line` 多認幾種 item type，映到既有 4 個 variant | **僅** `stream/codex_parser.rs`(+測試) |
| 2. 新增 StreamEvent variant | 加 `Error` / 區分 `AssistantText` vs `Thought` | render/stream_event.rs + app `ActivityStreamItem.tsx` / `ChatTranscript.tsx` + `log/events/jsonl_sink.rs` + `store/goals.ts` / `chat.ts`——**跨 Rust+TS 重改** |

**建議方案 1**，因為現有 variant 已夠用：
- **檔案編輯**：codex `apply_patch`（spike §219 確認 codex 內建此工具）→ 映成 `ToolUse{name:"Edit", input:{file_path}} + ToolResult`。`name:"Edit"` 會命中 render 的 `✍️ [正在生成]` 路徑（`stream_event.rs:52`），與 Claude 一致。
- **錯誤/失敗**：`turn.failed` / error item → `ToolResult{output:<訊息>, is_error:true}`（命中 `👀 [觀察結果]` 路徑），失敗不再靜默。
- **附帶修正**：`command_execution` 目前 `name:"Shell"`（`codex_parser.rs:39`），spike 原規劃是 `"Bash"`（doc §54）——統一成 `"Bash"` 與 Claude 工具命名對齊（純字串改）。

### ⚠️ 阻塞：缺 ground-truth event 樣本（BLOCKED 項，留給 harry / 實測）
2026-05-22 spike 只錄了 `command_execution` / `agent_message` / `turn.completed` 三種樣本（backlog §43-45, §125-129）。**從未錄到** apply_patch 編輯、turn.failed 的實際 `--json` event 形狀。所以方案 1 的精確 key 名（item type 字串、欄位名）**無法只靠讀碼確定**，需一次真實 codex 跑：
- (a) 在 workspace-write 下讓 codex 改一個檔，錄 `--json`；
- (b) 故意觸發失敗（如 Azure 404 / sandbox 拒寫），錄 `--json`。

**設計不被此阻塞，實作被此阻塞。** PE2 產出此設計即 DONE；轉成實作 change 前需先補這兩個樣本。

### 受影響檔案 / 工程量
- `codebus-core/src/stream/codex_parser.rs`(+測試)。
- 工程量：**輕-中（半天～1 個半天）**，視 codex event 形狀複雜度；零跨 crate（這正是選方案 1 的價值）。

---

## 未決問題（需 harry / 實測）

1. **【最高優先，PE1 已問】** 「不理想」是**過程顯示空/亂**（→ 先做 C2）還是**最終答案內容差**（→ 先做 C1，且可能牽出 C3 模型行為）？這決定 C1/C2 誰先做。
2. **quiz Mode B 在 codex 下能否自我驗證？** `codebus quiz validate` 是讀 stdin 的 shell 命令，在 `-s read-only` sandbox 下 codex 是否允許執行（command exec vs 純讀）未知。若被擋 → quiz 在 codex 下無法自我驗證，需 B 方案給 codex 專屬指示（或改 quiz 在 codex 走不同驗證路徑）。**需實測。**
3. C2 的兩個 ground-truth 樣本（見上方 BLOCKED）。

## 建議落地順序（待 harry 解未決問題 1 後啟動，需解除「只讀」邊界）

1. 補 C2 ground-truth 樣本（短，需真實 codex）。
2. C2 方案 1：擴 codex parser（輕-中、低風險、收益最直接——讓 codex「做了什麼/失敗了沒」可見）。
3. C1 方案 A：skill 機制無關化（輕、修正確性）。
4. 視 quiz 實測（未決 2）決定是否要 C1 方案 B 的局部 codex 分支。

---

## 給後續的交接摘要

- 動 **parser（C2 方案 1）** 與 **skill 文字（C1 方案 A）** 兩者皆**不需碰 render / app / CLAUDE.md / AGENTS.md**，blast radius 小。
- 真正需要 harry 的是：**未決問題 1 的一句話回答** + **一次真實 codex 跑**錄兩個 event 樣本。
- 若日後要追求 codex 答案的逐字串流（增量輸出）或語意化工具名，才需要動 `StreamEvent` enum（方案 2，跨 Rust+TS 重改）——非當前優先。
