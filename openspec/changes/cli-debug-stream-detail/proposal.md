## Summary

`codebus --debug` 開啟時，CLI 的 agent stream 渲染（`format_event`）改為「不截斷」：顯示完整 tool input 與完整 tool result，讓使用者能看到 agent 實際怎麼用工具、拿到什麼結果；預設（無 `--debug`）維持現有精簡渲染不變。

## Motivation

目前 CLI 跑 goal/query/fix/chat 時，stream 上的 agent 活動是**精簡/截斷**後才顯示（`codebus-core/src/render/stream_event.rs::format_event`）：

- 一般 tool：`🛠️ name(arg 摘要)`，input 經 `format_tool_args` / `short_value` 壓縮——陣列收成 `[N items]`、物件收成 `{N keys}`，複雜參數看不到
- Write/Edit：只顯示 `✍️ [正在生成] <檔案路徑>`，寫入內容不顯示
- ToolResult：截斷到 200 字、Read 只印 `(N lines)`、Write 成功回應整個抑制

結果是觀察 agent 行為時，「思考」看得到（Thought 已完整渲染），但「工具怎麼用、拿到什麼」幾乎看不到細節。debug AI 行為或理解 agent 為何這樣做時，這些被截掉的正是需要的資訊。

`--debug` 在 `cli` spec 已是「顯示內部細節」的旗標（Debug Flag Output requirement——印 per-step `✓` 與 `[debug]` 內部 trace），語意上把「agent stream 完整細節」一併納入最自然，且不需新增旗標。

## Proposed Solution

1. `RenderOptions`（`codebus-core/src/render/options.rs`）新增 `pub verbose: bool` 欄位；既有 constructor（`detect` / `detect_with_vault_id` / `no_styling` / `explicit`）一律預設 `verbose: false`，不改 `explicit` 的 4-參數簽名（lint 等既有呼叫點不受影響）。
2. CLI 端（`codebus-cli/src/main.rs`）建好 `RenderOptions` 後，依 `cli.debug` 以欄位賦值設定 `verbose`（`--debug` → `verbose = true`），再傳給各 verb 的 `print_event` 渲染閉包。
3. `format_event`（`codebus-core/src/render/stream_event.rs`）依 `opts.verbose` 分支：
   - `verbose = false`（預設）：維持現有精簡/截斷行為，與今日逐字節相同。
   - `verbose = true`：ToolUse 顯示完整 input（含 Write/Edit 內容、複雜物件參數，以可讀 JSON）、ToolResult 顯示完整 output（不截 200 字、Read 不只印行數、不抑制 Write 成功回應）。Thought 兩種模式都完整（現況即如此）。

## Non-Goals

- App（codebus-app）端對應的完整顯示——已記於 backlog `app-stream-verbose-detail`（純前端 follow-up，後端資料已完整），不在本 change。
- raw claude stream-json dump（含 system/init/hook 等被正規化丟棄的事件）——另一個更大的「raw protocol 視圖」需求，不在本 change。
- 把 assistant 純文字回覆 surface 成新 StreamEvent 變體——本 change 只動既有四個變體的渲染詳細度，不擴 enum。
- 新增 `--verbose` / `-v` 旗標——複用既有 `--debug`，不 rename、不新增。
- 改變預設（無 `--debug`）模式的任何輸出——預設逐字節不變。

## Alternatives Considered

- **新增獨立 `--verbose` / `-v` 旗標**：軸分離較乾淨，但 `--debug` 在 cli spec 已定義為「顯示內部細節」、語意更貼切，rename 一個成文旗標屬破壞性變更、收益僅命名——rejected，複用 `--debug`。
- **預設就顯示完整 input/result**：完整 tool result 可能很大（讀檔、grep），預設全開會洗版——rejected，必須 gated，預設維持精簡。
- **`explicit()` 加 verbose 參數**：會破壞所有既有 4-參數呼叫點——rejected，改用 pub 欄位賦值 + 預設 false。

## Impact

- Affected specs:
  - cli（Debug Flag Output requirement——擴充：`--debug` 額外影響 stream 渲染詳細度）
  - agent-stream-rendering（Stream Event Terminal Rendering requirement——修改：截斷 / Read 行數 / Write echo 抑制改為「預設模式」行為，新增「verbose 模式完整渲染」scenario）
- Affected code:
  - Modified: codebus-core/src/render/options.rs（`RenderOptions` 加 `verbose` 欄位 + 對應測試）
  - Modified: codebus-core/src/render/stream_event.rs（`format_event` 加 verbose 分支 + 測試）
  - Modified: codebus-cli/src/main.rs（依 `cli.debug` 設 `RenderOptions.verbose`）
