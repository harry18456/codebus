# PE1 診斷：Codex 輸出不理想屬哪類成因

**Date:** 2026-05-22
**Task:** loop PE1（只讀診斷，不動實作）
**背景:** [provider-prompt-engineering backlog](2026-05-22-provider-prompt-engineering-backlog.md)
**方法:** 讀 agent / stream / skill_bundle 層原始碼，比對 claude vs codex 兩條路徑的指示材料通道與 stream 解析保真度。

---

## TL;DR

「輸出不理想」**最可能是兩類成因疊加，而非模型本身差**：

1. **指示材料不對味（prompt 層）** — skill bundle 與 `AGENTS.md` 對 codex 是 **byte-identical 沿用 Claude 內容**，裡面寫死了 Claude 專屬機制（`--tools Read,Glob,Grep`、PreToolUse hook、`mcp_*` 工具族）。對 codex 這些敘述**要嘛無效噪音、要嘛直接誤導**（最嚴重是 quiz 的自我驗證契約假設了一個 codex 根本沒有的 hook）。
2. **Stream parser 保真度（顯示層）** — codex 的真實工作（檔案編輯、錯誤/失敗、細粒度工具呼叫）在 activity stream 裡**大量不可見**，因為 `parse_codex_stream_line` 只映了三種 event。這讓 codex「看起來」產出比實際單薄，即使底層模型沒問題。

第三類「模型行為差異」**目前無法排除**，需 harry 提供具體例子（哪個 verb、輸入、codex 實際輸出 vs 期望）才能定位。

**一個結構性事實**：目前**完全沒有 per-provider 的 prompt 差異化縫**。`SpawnSpec.prompt` 由 verb 層組一次、刻意 provider-neutral（`spawn_spec.rs:11-17`），指示材料雙寫也是 byte-identical。所以就算想給 codex 不同寫法，現在沒有地方塞——這是 PE2 要設計的核心缺口。

---

## 證據

### A. 指示材料通道（prompt 層）

兩 provider 的指示來源：
- Claude：`<vault>/.codebus/CLAUDE.md` + `.codebus/.claude/skills/codebus-{verb}/SKILL.md`
- Codex：`<vault>/.codebus/AGENTS.md` + `.codebus/.codex/skills/codebus-{verb}/SKILL.md`

**關鍵：codex 的材料是 Claude 材料的逐字鏡射。**
- skill bundle：`write_codex_materialization_if_missing` 直接重用 `write_bundle_if_missing` + 同一個 `stub_content(verb)`（`skill_bundle/mod.rs:117-127`），明說「the same stub content is reused verbatim」（`:99-102`）。
- `AGENTS.md`：以 `agents_md_content` 寫入，註解明說鏡射 vault `CLAUDE.md`（`skill_bundle/mod.rs:110-116, 128-131`）。

**沿用內容裡對 codex 失準的具體敘述**（皆出自 `stub_content`）：

| 位置 | 沿用的 Claude 敘述 | 對 codex 的問題 |
|---|---|---|
| query SKILL `:293` | 「`--tools Read,Glob,Grep` was passed when this agent was spawned, so Write and Edit attempts will fail」 | codex 沒有 `--tools`；唯讀是靠 `-s read-only`（`codex_backend.rs:54-59,115-120`）。敘述的機制不存在 |
| chat SKILL `:337` | 「binary-layer toolset is gated at spawn time (`--tools Read,Glob,Grep`)」+ 禁 `mcp_*` 族 | 同上；且 codex 的 MCP 工具命名不同，`mcp_*` 前綴規則對 codex 是空談 |
| quiz SKILL `:418` | 「the sandbox **hook** only permits a Bash command whose first word is `codebus`」「the **PreToolUse hook** blocks it」 | codex 用 `--ignore-rules` + `-s` sandbox（`codex_backend.rs:94`），**沒有 PreToolUse hook**。quiz Mode B 的 `codebus quiz validate` 自我驗證契約建立在不存在的機制上 → 自我驗證很可能在 codex 下行為不符預期 |
| fix SKILL `:526` | 「The PreToolUse hook installed by `codebus init` permits `codebus lint *` and blocks any other Bash」 | 同上，codex 無此 hook |

→ 這些不是「語氣不同」，是**事實錯誤的機制描述**。對遵從度高的模型，描述一個不存在的安全機制會干擾它的工具使用決策。

### B. Stream parser 保真度（顯示層）

`StreamEvent` 只有 4 種：`Thought / ToolUse / ToolResult / Usage`（`parser.rs:41-48`）。**沒有獨立的「最終答案」variant**——Claude 的 `text` 與 codex 的 `agent_message` **都**映成 `Thought`（`parser.rs:118-125` vs `codex_parser.rs:46-53`）。所以「答案被當成 thought」這點兩邊一致，**不是** codex 獨有的問題（這修正了 backlog 裡的初步猜測）。

codex parser 真正的缺口（`codex_parser.rs:14-73`）——只處理 `item.completed`(command_execution / agent_message) + `turn.completed`(usage) + `thread.started`，其餘一律回空 vec：

1. **檔案編輯不可見（對 goal/fix 最致命）** — codex 在 `workspace-write` 下用 apply_patch / file_change 類 item 改檔，不是 `command_execution`。parser 沒處理這些 → goal/fix 實際寫了 wiki 卻在 stream 裡**毫無痕跡**，使用者看到「沒動作 → 突然一句結語」。
2. **錯誤/失敗被靜默吞掉** — 沒處理 `turn.failed` / error item。codex turn 失敗（Azure 404、sandbox 拒絕等，正是 `codex_backend.rs` 測試裡修過的「no response」類 bug）→ 回空 vec → 前端**沒有任何錯誤提示**。
3. **工具呼叫全塌成 "Shell"** — `command_execution` 一律 `name:"Shell"`（`codex_parser.rs:39`），不像 Claude 有 Read/Glob/Grep/Edit 等語意名稱，activity stream 資訊量低。
4. **無增量串流** — 只在 `item.completed` 吐，`item.started` 跳過（`codex_parser.rs` 測試 `:163-167`）→ 輸出一次到位、不像 Claude 逐塊出現，互動感差。
5. **header 註解自承是 stub** — `codex_parser.rs:5-6`「Real mapping implemented in task 3.4; these are stubs」——雖然下面已有實作，但確認此 mapping 是 MVP 級、未涵蓋完整 codex event taxonomy。

→ 即使 codex 模型輸出正常，(1)(2) 會讓 goal/fix 看起來「什麼都沒做又沒報錯」，(3)(4) 讓所有 verb 的過程顯示比 Claude 單薄。**這類問題改 parser 就能修，與 prompt 無關。**

---

## 成因歸類結論

| 類別 | 判定 | 證據強度 | 修復面向（PE2 設計） |
|---|---|---|---|
| 1. prompt/指示不對味 | **成立** | 高（A 表逐條） | per-provider skill/AGENTS 內容差異化 |
| 2. parser/顯示保真度 | **成立** | 高（B 1-5） | 擴 `parse_codex_stream_line` event 覆蓋 + 可能新增 StreamEvent variant |
| 3. 模型行為差異 | **未定** | 待 harry 樣本 | 視樣本決定是否需 per-provider prompt 調校 |

**最該先動的**：類別 2 的 (1)(2)——檔案編輯不可見、錯誤靜默——因為它們會讓使用者**誤判**模型能力，且修復不需碰 prompt、風險低、收益直接。類別 1 次之（影響正確性但較細）。

## 給 PE2 的交接

1. 設計 per-provider 指示材料差異化的縫（目前 `stub_content` 與 AGENTS 鏡射都 byte-identical，無 provider 參數）。
2. 設計 codex parser 的 event 覆蓋擴充（apply_patch/file_change、turn.failed/error、reasoning item），評估是否要動 `StreamEvent` enum（牽動兩 provider 的 render 層，屬較重改動）。
3. 待 harry 補具體「不理想」樣本後，再判類別 3 是否需要 per-provider prompt 內容（而非僅機制描述）的調校。

## 待 harry 補的料

具體案例：哪個 verb、輸入 prompt、codex 實際輸出、你期望的樣子。尤其想知道「不理想」是指**過程顯示空/亂**（→ 指向類別 2）還是**最終答案內容差**（→ 指向類別 1 或 3）——這兩者的修法完全不同。
