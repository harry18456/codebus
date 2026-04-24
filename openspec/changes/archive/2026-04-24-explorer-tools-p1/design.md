## Context

`explorer-tools-p0`（2026-04-24 archive）落地四個 P0 工具 —— `search` / `list_dir` / `read_file` / `mark_station` —— 全部在 `FolderTools`（`sidecar/src/codebus_agent/agent/tools/folder_tools.py`）的方法級呼叫，走 `ExplorerAction.tool_calls[*].name` → `_execute_one` 的字串 dispatch。P0 留了兩個**差異化武器**：`trace_import` / `find_callers`（`docs/agent-explorer-spec.md §九 P1`、`CLAUDE.md` 架構快照「Module 4 Explorer 核心」），目前 `ExplorerTools` Protocol 是 structural（`runtime_checkable` 但三個 abstractmethod 只宣告 P0 三主力），P1 工具靠「方法存在即能 dispatch」不用改 Protocol。

約束：

- **打包穩定性**：sidecar 走 PyInstaller onefile（D-014），新 runtime 依賴都得在 spec 和 pyproject lock 同步。Windows cross-platform build 對 C 擴充特別挑剔（tree-sitter 在 Windows 經常爆）。
- **Sandbox + Sanitize 紅線**：所有檔案 I/O 先過 `ensure_in_workspace`（invariant #6）；所有回傳給 Agent 的 content 字串過 Pass 1 sanitize（invariant #3）。`trace_import` 只回 path 不需 sanitize；`find_callers` 回含 snippet 必 sanitize。
- **稽核一致性**：P0 透過 `sandbox.append_tool_audit_line` 共用 writer 寫 `tool_audit.jsonl`，P1 沿用。
- **`tool_specs()` 合約**：P0 引入 optional `tool_specs()` 把工具介紹給 Explorer prompt；新增工具要同步加枚舉，否則 LLM 看不到也不會呼。

## Goals / Non-Goals

**Goals:**

- Explorer 能做「符號級導航」：拿到 symbol 名就能找 definition site（`trace_import`）或所有 call-site（`find_callers`）。
- 跨 MVP 目標的語言覆蓋（Python / TS / JS / Go / Rust / Markdown）— regex 一律不做 AST 精度承諾。
- 新工具的審計 / sandbox / sanitize 紀律**與 P0 完全對稱**（紅隊 fixture 覆蓋 path escape、symlink）。

**Non-Goals:**

- 見 proposal Non-Goals 段（tree-sitter / ast 特化 / symbol index 持久化 / TS import resolve / scoping / context-aware FP 過濾 / 前端整合）。
- **不**觸碰 `ExplorerTools` Protocol 宣告；新工具只存在於 `FolderTools` 實作層（structural 滿足）。
- **不**改 `_execute_one` 的 dispatch 邏輯 —— 名字對就走方法、對不上就 `ToolResult.error`。
- **不**做 fuzzy / stemming 符號比對（`foo_bar` 不會命中 `fooBar`）。

## Decisions

### Regex-based definition & call-site 比對 — 不引 AST / tree-sitter

`trace_import(symbol)` / `find_callers(symbol)` 的命中策略走「每個允許副檔名用共通的語言中性正則」。允許副檔名沿用 P0 search fallback 的 allowlist：`.py / .md / .ts / .tsx / .rs / .go / .js / .jsx`。

**Definition patterns**（`trace_import` 用；第一個命中即回傳）：

| Language family | Pattern | 命中例 |
|---|---|---|
| Python | `r"^\s*(?:async\s+)?def\s+<symbol>\b"` | `def Bar(`、`async def Bar(` |
| Python class | `r"^\s*class\s+<symbol>\b"` | `class Bar:` |
| TS / JS class | `r"^\s*(?:export\s+)?class\s+<symbol>\b"` | `class Bar {`、`export class Bar {` |
| TS / JS function | `r"^\s*(?:export\s+)?(?:async\s+)?function\s+<symbol>\b"` | `function Bar(` |
| TS / JS const | `r"^\s*(?:export\s+)?(?:const\|let\|var)\s+<symbol>\b"` | `export const Bar =` |
| Go func | `r"^\s*func\s+(?:\([^)]+\)\s+)?<symbol>\b"` | `func Bar(`、`func (r *R) Bar(` |
| Go type | `r"^\s*type\s+<symbol>\b"` | `type Bar struct` |
| Rust fn | `r"^\s*(?:pub\s+)?(?:async\s+)?fn\s+<symbol>\b"` | `fn Bar(`、`pub async fn Bar(` |
| Rust struct / enum / trait | `r"^\s*(?:pub\s+)?(?:struct\|enum\|trait)\s+<symbol>\b"` | `struct Bar`、`pub enum Bar` |

`<symbol>` 以 `re.escape(symbol)` 防注入（Agent 可能塞奇怪字串）。multiple patterns 合併成 `(?:p1)|(?:p2)|...` 一次掃每檔；比對完回第一個命中檔案的 relative path。

**Call-site pattern**（`find_callers` 用）：

- `r"\b<symbol>\b"`（whole-word 邊界）—— 意圖最大化召回，不過濾 definition line（由後處理剔除）。
- 跑完再過**後處理兩步**：
  1. 剔除 definition-line hit（和 `trace_import` 回的 path:line 對比）。
  2. 同一檔多個 hit 只保留前 5 個（避免 overflow 單檔灌爆上限）。

**為何不 AST / tree-sitter**：

- Python `ast` 只覆蓋 `.py`，其他語言還是得 regex；雙碼路維護成本 > 一致 regex 的命中率損失。
- tree-sitter Windows PyInstaller 打包爆炸率高（native library 路徑解析），MVP 期不值得。
- P0 demo fixture 規模下，regex 命中率「看得到 def / class / function 就行」已足夠撐 Demo 敘事。

**替代方案** — Python 特化 AST：棄用，理由如上。

### FileMatch schema — 輕量、只放 Agent 需要的

新增 `FileMatch(path: str, line: int, snippet: str)` Pydantic model。`path` 相對 workspace root、`line` 1-indexed、`snippet` 單行原文過 Pass 1 sanitize 後截到 200 字。**不**多加 `column` / `end_line` / `ast_node` 等 metadata —— Agent 只需要「在哪個檔案哪行看到」，其他資訊要就叫它自己 `read_file` 追。

### 回傳格式 — `trace_import` 回 single path，`find_callers` 回 list ≤ 100

`trace_import` 回 `str | None`。找不到 → `None`。多候選 → 取 **deterministic 排序後的第一個命中**（排序鍵：`(path_depth, path_str)`，確保 `src/foo.py` 先於 `tests/foo.py`）。這個設計讓 Agent 下一步能直接 `read_file(<path>)`，不用看列表選。

`find_callers` 回 `list[FileMatch]`，上限 100（和 `search` 對齊）。**排除** `trace_import` 會回的 definition 行（比對 `(path, line)` tuple）—— 同一工具集裡不重複訊息。單檔上限 5 個 hit（避免 overflow / snippet storm）。排序：`(path_depth, path_str, line)`。

### sanitize 策略 — 只有 `find_callers` 要，`trace_import` 不要

`trace_import` 回 path 字串，不含程式碼內容，不走 sanitize。`find_callers` 的 `snippet` 是程式碼行，走 `ctx.sanitizer.sanitize(...)` Pass 1；命中一樣進 `sanitize_audit.jsonl`（`pass_num=1`）。`ctx.sanitizer is None` → fail-loud 與 `read_file` 一致（`ValueError`）。

### 掃描實作 — async path iteration + early-exit for trace_import

兩工具實作都走 `async` method。但 I/O 主要是 read-text，Python 檔案 iteration 沒 true async 收益；`asyncio.to_thread` 也被 P0 工具忽略為 over-engineering。實作照 P0 風格用同步 `Path.read_text()`，method 本體 `async def`，呼叫時 await 空殼。

`trace_import` 一旦命中第一個 match 立即 break iteration 並回傳（early exit）；`find_callers` 掃完全部允許副檔名。兩工具都**尊重 `search` fallback 的 gitignore / binary / too-large 過濾邏輯** —— 實作上直接呼叫既有的 scanner walker 介面（`codebus_agent.scanner` 的 text-file filter），避免重複。

### 紅隊覆蓋 — path escape / symlink 必測

新測試層必含：

- `trace_import("Symbol")` 在 workspace 裡放一個 symlink 指向外部檔案（含 definition）→ `ensure_in_workspace` 拒絕 → `None`（不是該檔路徑）。
- `find_callers("Symbol")` 的 hit path 組裝後若 `ensure_in_workspace` 拒絕 → 跳過並寫 `tool_audit.jsonl` `allowed=false`。
- 與 P0 `test_folder_tools_audit.py` 格式一致，共用 `append_tool_audit_line`。

## Risks / Trade-offs

- [regex 命中率 < AST] → Python 重度 metaprogramming 的 repo 可能漏命中。**Mitigation**：限定 P1 場景為 Demo 合成 fixture；打磨期如果有真 repo 驗證命中率不足，再開獨立 change 引入 AST 特化。

- [`find_callers` whole-word 會命中字串字面值 / 注解] → False positive 增加 snippet 噪音。**Mitigation**：單檔上限 5 + 全域上限 100 + Pass 1 sanitize 先過；Agent 看到大量同檔命中會自行放棄追（Judge prompt 判 relevance）。不做 context-aware 過濾避免 over-engineering。

- [單一符號名在大型 codebase 極多 hit] → 100 hit 上限切掉尾端有用的命中。**Mitigation**：排序鍵用 `(path_depth, path_str, line)` 讓 `src/` 淺路徑先命中；100 是實作上限，打磨期可加 priority scoring。

- [multiple deftinitions 的語言（TS `declare`、Python dunder 多個檔案都 def 同名）] → `trace_import` 取第一個命中可能不是「真正的」定義。**Mitigation**：接受 P1 精度損失，Agent 若看 path 不對可叫 `search(symbol)` 拿替代。

- [Windows 的 line ending / encoding 混亂] → regex 命中 `^\s*def` 時 `\r\n` 可能破壞。**Mitigation**：檔案一律用既有 scanner 的 `charset-normalizer` 讀 + `splitlines()`（跨平台 line-end 吞掉），符合 P0 `search` 實作紀律。

- [兩工具都掃全 workspace] → 大型 repo 延遲高。**Mitigation**：Non-Goal 列 symbol index 持久化；目標 demo fixture < 50 檔秒回；Agent budget 控制能擋太多呼叫。

## Migration Plan

無破壞性改動 —— 純新增 method + 新增 schema。既有 P0 工具不動；`ExplorerTools` Protocol 不動。既有 `test_explorer_loop.py` 的「Unknown tool name」scenario 需把 placeholder 從 `trace_import` 換成 `find_nonexistent`（同功能不同名），避免「已實作的工具」被當作 fallback 測 fixture。

## Open Questions

無。（proposal + design 已覆蓋本 change 的所有決策面。）
