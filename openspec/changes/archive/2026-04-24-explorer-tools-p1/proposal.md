## Why

`explorer-tools-p0`（2026-04-24）落地了 Module 4 Explorer 的四個 P0 工具（`search` / `list_dir` / `read_file` / `mark_station`），但**差異化武器層**（`docs/agent-explorer-spec.md §九 P1` 的 `trace_import` / `find_callers`）還沒動。現況 Explorer 只能「字串級搜尋 + 讀檔」，當它看到 `from foo import Bar` 或 `Bar()` 呼叫時**沒有任何結構化方式追依賴或反查呼叫點** —— 必須靠 `search("Bar")` 拿到一堆雜訊、再一個個 `read_file` 才能拼出依賴圖。對齊 `docs/implementation-plan.md` **步驟 19**（`1d`，依賴步驟 17），本 change 把 P1 兩個工具補上，讓 Explorer 能做到 `agent-explorer-spec.md §五` 示範流程裡的「🔗 trace_import('MQTTClient') → src/mqtt/client.py」一類「符號級導航」，把 P0 的字串搜尋升級成有向圖遍歷。

對齊 **D-011**（自帶 sandbox + sanitize，所有工具產出必過 `ensure_in_workspace`）、**D-015**（Sanitizer 三段鏈，Pass 1 在工具邊界注入）、**D-017**（ToolSandbox 雙層稽核 `tool_audit.jsonl`）。

## What Changes

**新增兩個 Folder-mode 工具**（掛在既有 `FolderTools` / `explorer-tools` capability 上）：

- `trace_import(symbol: str) -> str | None`：回傳 `symbol` 被**定義**的檔案相對路徑；找不到回 `None`。作法：對 workspace 內允許副檔名（沿用 `search` grep fallback 的 `.py / .md / .ts / .tsx / .rs / .go / .js / .jsx`）掃 definition-site 正則（`def <symbol>` / `class <symbol>` / `function <symbol>` / `const <symbol> =` 等語言中性 pattern），取第一個命中的 path。預期主用於 Python 為主的目標 repo；跨語言命中靠 regex 最 generic 形式，不追求 AST 級精度（Non-Goal）。
- `find_callers(symbol: str) -> list[FileMatch]`：掃同一組副檔名裡「以 whole-word 形式出現」的 `symbol` call-site，回 `FileMatch(path, line, snippet)` 列表（`path` 相對 workspace root、`line` 1-indexed、`snippet` 單行 ≤ 200 字並過 Pass 1 sanitize）。上限 100 hits，**排除 definition site**（避免和 `trace_import` 結果撞）。

**擴充 `ToolResult` / protocol 層**：

- `FolderTools` 多兩個 async method；`ExplorerTools` Protocol 結構性滿足（既有 `runtime_checkable` 不變）。
- `ExplorerTools.tool_specs()` optional method 擴充到 6 個工具，讓 Explorer prompt 看見新武器。
- `explorer-tools` 既有 Requirement「Unknown tool name yields ToolResult.error without raising」中**將 `trace_import` 從「P1 未實作所以走 error 分支」移除**（改為實作可達；仍保留對未知工具的 error fallback）。

**稽核與 sandbox**：

- `trace_import` / `find_callers` 在第一次 path 組裝前、回傳前都過 `ensure_in_workspace(path, ctx)`。找到的結果 path 若落在 workspace 外（不應該發生，但 symlink 紅隊可能）直接略過並在 `tool_audit.jsonl` 寫 `allowed=false`。
- `find_callers` 的 `snippet` 過 Pass 1 sanitize（`ctx.sanitizer.sanitize(...)`）；`sanitize_audit.jsonl` 照寫。`trace_import` 只回 path，不涉及 sanitize。
- 兩工具都走 `sandbox.append_tool_audit_line` 寫 `tool_audit.jsonl`（和 P0 工具同格式）。

**Schema 新增** `FileMatch` Pydantic model（`sidecar/src/codebus_agent/agent/tools/schemas.py`），欄位 `path: str / line: int / snippet: str`。

**測試層**：

- `sidecar/tests/agent/tools/test_trace_import.py` —— definition site 命中（Python `def` / `class`、TS `function` / `class` / `export const`）、找不到時回 `None`、多候選時取第一個命中、跨副檔名覆蓋、路徑 escape 紅隊 fixture（sandbox.ensure_in_workspace 要擋）。
- `sidecar/tests/agent/tools/test_find_callers.py` —— 多呼叫點命中 + snippet sanitize、whole-word 邊界（`foo` 不該命中 `foobar`）、上限 100、排除 definition、上限以上正確截斷、Pass 1 命中要進 `sanitize_audit.jsonl`、紅隊 fixture。
- 既有 `test_folder_tools_structural.py` / `test_tool_specs.py` 補兩個新工具的 dispatch / spec 枚舉。
- 既有 `test_explorer_loop.py` 的 "Unknown tool name" scenario 要更新（改用 `find_nonexistent` 當 placeholder 未知工具，避免和 `trace_import` 撞）。

## Non-Goals

明確排除（留給後續 change 或打磨期）：

- **Tree-sitter / ast-grep 依賴**：不為了精度拉進新 parser 依賴；regex 命中率對 P1 夠用（Demo 靈魂只在 Python 合成 fixture 上跑）。升級到 AST 留給打磨期獨立 change。
- **Python `ast` 模組特化**：即便 sidecar 本身 Python，P1 仍走 regex 統一路徑，避免 Python-only / 其他語言 regex 的雙碼路。未來若 Python-heavy repo 命中率不足，獨立 change 引入 `ast.NodeVisitor` 版本。
- **Symbol index 持久化 / 快取**：每次工具呼叫都重掃 workspace。P0 目標 fixture（< 50 檔）秒回；大型 repo 效能問題留給步驟 21 context 壓縮同期解決。
- **跨語言 import resolve**：TS `import { Bar } from './foo'` 的 `./foo` → `./foo.ts` / `./foo/index.ts` 解析不做；只找 `Bar` 的 definition 字串命中。
- **Scoping / namespace / 方法解析**：`foo.Bar()` 只比對 `Bar`，不處理 `foo` 的模組別名或 class method 歸屬。
- **False-positive 降低**：whole-word 邊界靠 `\b` regex，不做 context-aware 過濾（例如排除注解 / 字串字面值）。
- **新 tool method 加進 `ExplorerTools` Protocol 宣告**：P0 已把 P1 工具留成 structural（未在 Protocol 宣告即可滿足），本 change 只往 `FolderTools` 加方法 + 往 `tool_specs()` 加枚舉。
- **前端 Agent console 整合**：步驟 28 / 28.5 範疇；本 change 不動前端。

**拒絕的設計**：

- **「P1 立刻上 tree-sitter」**：依賴重、build graph 複雜、Windows 打包 PyInstaller 常爆；Demo fixture 規模下沒必要。
- **「trace_import 回多個候選」**：Agent 需要一個具體路徑走下一步；回 list 反而讓 prompt 複雜度爆炸。多候選 P1 策略：取第一個命中（deterministic 排序），其餘丟掉。若 Agent 需要看替代候選，靠 `search(symbol)` 已能補位。
- **「find_callers 不做 snippet sanitize」**：P0 `read_file` 已建立「LLM 看到的一定是 sanitize 過的」紅線（invariant #3），跨工具統一紀律。

## Capabilities

### New Capabilities

（無 —— 兩個新工具掛在既有 `explorer-tools` capability 上）

### Modified Capabilities

- `explorer-tools`：新增 `trace_import` / `find_callers` 兩個 Requirement；既有「Unknown tool name yields ToolResult.error without raising」scenario 改引用其他 placeholder 工具名（因為 `trace_import` 已落地）。

## Impact

**受影響 spec**：

- `openspec/specs/explorer-tools/spec.md`（MODIFIED — 新增 2 個 Requirement + 修正 1 個既有 scenario 的 placeholder）

**受影響 code**：

- `sidecar/src/codebus_agent/agent/tools/folder_tools.py`（新增 `trace_import` / `find_callers` 兩個 async method）
- `sidecar/src/codebus_agent/agent/tools/schemas.py`（新增 `FileMatch` Pydantic model）
- `sidecar/src/codebus_agent/agent/tools/__init__.py`（re-export `FileMatch`；`tool_specs()` 擴充）

**受影響測試**：

- 新：`sidecar/tests/agent/tools/test_trace_import.py`
- 新：`sidecar/tests/agent/tools/test_find_callers.py`
- 更新：`sidecar/tests/agent/tools/test_folder_tools_structural.py`（加 P1 工具 dispatch 測試）
- 更新：`sidecar/tests/agent/tools/test_tool_specs.py`（枚舉 6 個工具）
- 更新：`sidecar/tests/agent/test_explorer_loop.py`（"Unknown tool name" 改 placeholder）

**受影響文件**：

- `CLAUDE.md`（archive 時間軸）
- `docs/agent-explorer-spec.md §九`（`trace_import` / `find_callers` 狀態表：⏳ P1 → ✅ 步驟 19 落地）
- `docs/tool-sandbox.md`（若需補 P1 工具的稽核 row；視內容已蓋到視情況）

**無新依賴**（`re` / `pathlib` / `pathspec` 皆既有）。
