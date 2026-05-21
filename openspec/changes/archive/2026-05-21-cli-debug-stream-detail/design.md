## Context

CLI 跑 verb 時，`agent::invoke` 把 `StreamEvent` 餵給 caller 的 `on_event` 閉包；CLI 的閉包呼叫 `print_event(&e, &render_opts)` → `format_event`（`codebus-core/src/render/stream_event.rs`）渲染到 stdout。`format_event` 目前對 tool 活動做精簡 / 截斷：

- `ToolUse`（一般）：`format_tool_args` / `short_value` 把 input 壓縮（陣列→`[N items]`、物件→`{N keys}`）
- `ToolUse`（Write/Edit）：只印 `file_path`，不印寫入內容
- `ToolResult`：>200 字截斷 + `…`、Read 只印 `(N lines)`、Write 成功 echo 抑制為空字串
- `Thought`：完整渲染（兩種模式都一樣）；`Usage`：空字串

這些截斷在 `agent-stream-rendering` 的 `Stream Event Terminal Rendering` requirement 是寫死的 SHALL。`RenderOptions`（`codebus-core/src/render/options.rs`）是 plain struct（pub 欄位），由 `RenderOptions::detect()` 在 verb 進入點建一次；CLI 另把 `cli.debug` 以獨立參數傳給各 `commands::*::run`。`--debug` 在 `cli` 的 `Debug Flag Output` requirement 已定義為「顯示內部細節」（per-step `✓` + `[debug]` trace）。

## Goals / Non-Goals

**Goals:**

- `--debug` 時 CLI agent stream 顯示完整 tool input + 完整 tool result，便於觀察 / debug agent 行為。
- 預設（無 `--debug`）逐字節維持現有渲染。
- 不新增旗標、不破壞既有 `RenderOptions` 呼叫點。

**Non-Goals:**

- App 端完整顯示（backlog `app-stream-verbose-detail`）。
- raw claude stream-json（system/init/hook 等被丟棄事件）的呈現。
- 把 assistant 純文字 surface 成新 `StreamEvent` 變體（不擴 enum）。
- 新增 `--verbose` / `-v`。
- 改變預設模式輸出。

## Decisions

### 複用 `--debug`，不新增旗標

`--debug` 在 cli spec 已是「顯示內部細節」旗標，語意涵蓋「agent stream 完整細節」最自然；rename 成 `--verbose` 屬破壞性變更、收益僅命名。

Alternatives：獨立 `--verbose`/`-v`（軸分離但要動成文旗標）—— rejected。

### `RenderOptions` 加 `pub verbose: bool`，欄位賦值串入

`RenderOptions` 新增 `pub verbose: bool`；`detect` / `detect_with_vault_id` / `no_styling` / `explicit` 一律建為 `verbose: false`，**不改 `explicit` 的 4-參數簽名**（lint 等既有呼叫點不動）。CLI `main.rs` 建好 `RenderOptions` 後，依 `cli.debug` 以 `render_opts.verbose = cli.debug` 設定，再傳給各 verb。

Alternatives：`explicit()` 加 verbose 參數—— rejected（破壞所有 4-參數呼叫點）。

### `format_event` 依 `opts.verbose` 分支；verbose = 完整 input/result

- `verbose = false`（預設）：維持現狀（含 Write/Edit 只印 path、200 字截斷、Read 行數、Write echo 抑制）。
- `verbose = true`：
  - 所有 `ToolUse`（含 Write/Edit）：印 `<name>` + 完整 `input` 的 JSON（pretty-printed、四格縮排）。完整涵蓋 Write/Edit 的寫入內容與複雜物件參數。
  - `ToolResult`：印完整 `output`（不截 200 字、不替換成 Read 行數、不抑制 Write echo），四格縮排。
  - `Thought`：兩模式皆完整（不變）；`Usage`：兩模式皆空（不變）。

Alternatives：預設就全展開—— rejected（大輸出洗版，必須 gated）。

## Implementation Contract

**Behavior:**

`codebus <verb> --debug` 跑 agent 時，stdout 的 stream 顯示每個 tool 呼叫的完整 input（含 Write/Edit 內容）與每個 tool 結果的完整 output（不截斷）。不帶 `--debug` 時，stream 渲染與今日逐字節相同。

**Interface / data shape:**

- `RenderOptions`（`codebus-core/src/render/options.rs`）新增公開欄位 `pub verbose: bool`。`detect()` / `detect_with_vault_id()` / `no_styling()` / `explicit()` 產出的值其 `verbose` 皆為 `false`。`explicit()` 簽名不變（仍 4 參數）。
- `format_event(&StreamEvent, &RenderOptions) -> String`（`codebus-core/src/render/stream_event.rs`）依 `opts.verbose` 分支，行為如上 Decision。
- `codebus-cli/src/main.rs`：建立 `RenderOptions` 後設 `render_opts.verbose = cli.debug`（`cli.debug` 為既有全域旗標）。

**Failure modes:**

- 無新錯誤路徑；`format_event` 仍回 `String`（verbose 空字串規則僅保留給 `Usage`）。verbose 模式下 Write echo 不再抑制（會顯示），屬預期行為差異而非錯誤。

**Acceptance criteria:**

- `codebus-core` 單元測試：對同一組 `StreamEvent`，`verbose=false` 的 `format_event` 輸出與既有測試逐字節相同（回歸鎖定）；`verbose=true` 時——ToolResult 500 字輸出不被截斷（完整出現、無 `…` 截斷尾）、Read 結果顯示完整 output 而非 `(N lines)`、Write 成功 echo 不為空字串、Write/Edit ToolUse 的 input 內容（如 file content）出現在輸出。
- `RenderOptions` 測試：`detect()` / `no_styling()` 的 `verbose` 為 false。
- `cargo test --package codebus-core` 全綠；`cargo build --package codebus-cli` 通過；既有 CLI 整合測試不因此 fail。

**Scope boundaries:**

In scope：`RenderOptions.verbose` 欄位 + `format_event` verbose 分支 + `main.rs` 串入 `cli.debug` + 對應測試 + `cli` / `agent-stream-rendering` 兩條 spec delta（MODIFIED）。

Out of scope：app 前端、raw stream-json、assistant 純文字變體、新旗標、預設模式行為。

## Risks / Trade-offs

- [verbose 下完整 tool result 可能很大（讀大檔 / grep 一堆），洗版] → Mitigation: 僅 `--debug` 時開啟，預設精簡不變；這正是 debug 模式刻意要的完整資訊。
- [既有 `format_event` 測試逐字節斷言] → Mitigation: verbose 分支為新增；預設路徑不動，既有測試應全數續綠，並新增 verbose 專屬測試。
- [`RenderOptions` 加欄位影響 struct literal 建構點] → Mitigation: 僅 options.rs 內部 constructor 需補 `verbose: false`；`explicit()` 簽名不變故外部呼叫點零改動。
