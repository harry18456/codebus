## Context

codebus 用 `agent::invoke`（`codebus-core/src/agent/claude_cli.rs`）跑 provider-agnostic 的 spawn 迴圈，透過 `AgentBackend` trait 委派 argv 組裝 / stream 解析 / session-id 抽取。三個既有缺口環繞「spawn / 結果解析 / config 收尾正確性」：

- **Token 累加**：迴圈對每個 `StreamEvent::Usage` 呼叫 `accumulate_token_usage`（欄位 `saturating_add`）。claude `result.usage` 一個 `-p` run 只 emit 一筆（求和無誤）；codex `turn.completed.usage` 是**累計總量**，多筆即 double-count。`StreamEvent` 經 `VerbEvent::Stream` 寫進 events.jsonl、`TokenUsage` 寫進 runs.jsonl —— 兩者序列化形狀不可破壞。
- **Session 持久化**：codex backend 對非 `Chat` verb 加 `--ephemeral`；claude backend `compose_claude_cmd` 無對應旗標，goal/query/fix/quiz 每跑必落地 session 檔。
- **Effort 校驗**：`SystemVerbConfig.effort` / `AzureVerbConfig.effort` 是無校驗 `String`；閉集只活在前端 `ipc.ts` 的 `SYSTEM_EFFORTS`。

關鍵 invariant（`agent-backend` spec「Invocation Loop Drives Backend Trait」/「Polling mechanism is provider-agnostic」）：`invoke` 迴圈內不得引用 provider 名 / provider argv flag / provider stream-json 欄位，且任何 invoke 內機制須對每個 `&dyn AgentBackend` 一致適用。

**Ground-truth（effort 閉集 = 5 值，`auto` 不合法）**：`claude --help` 實證 `--effort <level>` 只接受 `(low, medium, high, xhigh, max)` —— **5 值、不含 `auto`**。現況的兩個錯誤：(1) 前端 `SYSTEM_EFFORTS` 含 `auto`（6 值）且上方註解宣稱「mirrors … --effort accepted set (… / auto)」，與 CLI 不符；(2) `compose_claude_cmd` 把 effort 原樣丟 `--effort`、不特判，使用者選 `auto` → 送 `--effort auto` → CLI 拒 → **spawn 失敗**（`auto` 從未能成功用）。Part C 因此把閉集定為 **5 值**、`auto` 視為不合法，並同步修正 GUI（`SYSTEM_EFFORTS` 移除 `auto` + 修註解），讓 loader / GUI / CLI 三方一致。**不走 sentinel（不做 `auto`→omit `--effort` 的 backend 特判）。**

另：`claude-code-config` spec §「System Profile Model Aliases」宣稱 `SystemModel` 是封閉 4-variant enum、拒絕 `gpt-4`，但 code 實際是 `model: String` + `system_model_to_cli_flag` 只加 `claude-` 前綴（無 `SystemModel` enum 存在）—— 此為既有 spec/code drift，**屬另一條 BACKLOG、不在本 change 範圍**，既有「Invalid SystemModel value rejected」scenario 照原樣保留、model 寬鬆 forward-compat 不動。

## Goals / Non-Goals

**Goals:**

- Part A：codex 的 cumulative token usage 不再被求和重複計數；`invoke` 對 provider 保持中立。
- Part B：claude 非 chat verb 不再落地 orphan session 檔；chat 維持持久化讓 resume 可用。
- Part C：非法 claude effort 在 Rust loader 階段即被 `ConfigLoadError` 擋下（作用中 profile）。

**Non-Goals:**

- 不閉集化 `model`（維持 forward-compat 自由字串）。
- 不動 codex effort。
- 不校驗 cold-storage（非作用中）profile 的 effort。
- 不修上述 `SystemModel` spec/code drift。
- 不在本 change 跑真 codex 多步 exec 把 Part A 從 latent 升 live。

## Decisions

### D1（Part A）以 backend trait 方法宣告 usage 語意，invoke 多型分派

`AgentBackend` 新增 opt-in 預設方法 `fn token_usage_semantics(&self) -> TokenUsageSemantics { TokenUsageSemantics::Delta }`（與既有 `stdin_payload` 同「預設 + 選擇性 override」模式）。`ClaudeBackend` 用預設（`Delta`）；`CodexBackend` override 回 `Cumulative`。`invoke` 在迴圈外取一次語意，迴圈內以 provider-neutral 的 `apply_token_usage(&mut acc, u, semantics)` 取代直呼 `accumulate_token_usage`：

- `Delta` → 既有 `accumulate_token_usage`（欄位 saturating 求和）。
- `Cumulative` → last-wins 覆寫（`*acc = addend.clone()`）：每筆 cumulative 事件帶當下累計總量，最後一筆即最終值。

`TokenUsageSemantics`（`#[derive(Debug, Clone, Copy, PartialEq, Eq)]`，無 serde derive）定義於 `log` 模組（與 `TokenUsage` 同處），`log` 為 leaf module、不 import `agent`，故 `backend.rs`／`sink.rs` 皆可引用無循環依賴。

**為何不選 parser 夾帶語意於事件中**：`StreamEvent::Usage` 持久化至 events.jsonl、`TokenUsage` 持久化至 runs.jsonl；在其上加欄位／改 variant 形狀會破壞 jsonl 相容性，且把只有 invoke 收尾用得到的 transient 語意污染進持久化 DTO。trait 方法把語意留在 backend（provider 知識歸屬地），invoke 以 enum 分派保持中立，零持久化形狀變動。`invoke` 只 `match` 語意 enum，從不出現 `codex`/`claude` 字面 —— 滿足 provider-agnostic invariant。

**為何 last-wins 用整體覆寫**：cumulative 來源各 Option 欄位（cache_read 等）亦累計、單調遞增，整體覆寫最後一筆最簡且正確，無需逐欄合併。

`accumulate_token_usage` 保留為 `Delta` 實作，且 `verb/quiz.rs` 跨多次 spawn 累加 `accumulated_tokens` 仍直接沿用（不同 spawn 之和為真加總、合法不受影響）。

### D2（Part B）以 verb gate 決定 claude session 持久化

`compose_claude_cmd` 增一個布林參數（語意：是否加 `--no-session-persistence`），由 `ClaudeBackend::build_command` 以 `!matches!(spec.verb, Verb::Chat)` 計算後傳入。為 `true` 時於穩定 argv 位置加入 `--no-session-persistence`（單一旗標、無值）。鏡像 codex backend 既有的 `Verb::Chat` `--ephemeral` gate。`--no-session-persistence` 僅在 `-p` 模式有效，codebus 一律用 `-p`。chat 不加旗標、`--resume` 路徑不受影響。

**Argv 位置**：置於 `--verbose` 之後、MCP isolation 旗標（`--strict-mcp-config`）之前，作為穩定可測位置；新位置寫入 `compose_claude_cmd` doc 的 argv order 清單。

### D3（Part C）effort 閉集 = 5 值（移除 `auto`），只校作用中 profile，GUI 同步

**閉集為何是 5 值而非保留 `auto`**：`claude --help` 實證 `--effort` 只收 `low/medium/high/xhigh/max`。`auto` 不在其中——使用者選 `auto` 時 `compose_claude_cmd` 原樣送 `--effort auto`、CLI 直接拒、spawn 失敗，所以 `auto` 今天就是壞掉、從未能用。故決定**移除 `auto` 而非保留**（也不走「`auto`→ backend 省略 `--effort`」的 sentinel 路線：那會新增特判、把一個本不該存在的值合理化）。

新增 `const CLAUDE_EFFORTS: [&str; 5] = ["low","medium","high","xhigh","max"]` 與 `fn validate_effort(effort: &str, field: &str) -> Result<(), ConfigLoadError>`（非法含 `auto` 一律回 `ConfigLoadError::YamlParse`，沿用既有 `serde_yaml::Error::custom` 錯誤建構模式、訊息含欄位路徑與允許集）。

於 `validate_system_profile` 的 `active=System` 分支、`validate_azure_profile` 的 `active=Azure` 分支，對 goal/query/fix/verify 四 verb 的 effort 各呼叫 `validate_effort`。**只校作用中 profile**：遵循 `claude-code-config` spec「codebus SHALL NOT validate fields of the non-active profile」。前端 `validateClaudeCodeBlock` 在 Save 時校兩 profile 為 UI 層 defense-in-depth；core loader 只把關實際會送進 `--effort` 的 active profile，避免「parked 在 cold storage 的 legacy effort 值阻擋整份 config 載入」的行為退步。此為刻意與前端不同範圍的決定。

**GUI 同步（同一 change 內）**：`codebus-app/src/lib/ipc.ts` 的 `SYSTEM_EFFORTS` 由 6 值改為 5 值（移除 `auto`）、修正其上方錯註解（改成「CLI accepts low/medium/high/xhigh/max」、移除 auto sentinel 描述、並反映 core loader 現在也校驗）；`validateClaudeCodeBlock` 讀 `SYSTEM_EFFORTS` 故自動只認 5 值；`codebus-app/src/lib/ipc.effort.test.ts` 由「six values」更新為「five values」、移除 `auto` 斷言。`EndpointSection.tsx` 的 dropdown 渲染 `SYSTEM_EFFORTS.map(...)` 故自動只剩 5 項、無需改。

**只校 claude；model 不動**。

## Implementation Contract

**Part A — observable behavior**：對一個 emit ≥2 筆 cumulative `turn.completed` 的 codex spawn，其 `RunLog.tokens` 等於**最後一筆**累計快照，而非各筆之和。claude（每 run 單筆 `result`）行為不變。`invoke` 迴圈內無任何 provider 名／argv flag／stream-json 欄位字面。
- Interface：`AgentBackend::token_usage_semantics(&self) -> TokenUsageSemantics`（預設 `Delta`）；`log::TokenUsageSemantics { Delta, Cumulative }`；`log::sink::apply_token_usage(acc: &mut TokenUsage, addend: &TokenUsage, semantics: TokenUsageSemantics)`。`accumulate_token_usage` 簽章與行為不變。
- Failure/edge：`Cumulative` 下 acc 起始為 `TokenUsage::default()`，零筆事件 → 維持 default（與現況一致）。
- Acceptance：新單元測試 —— 餵兩筆 codex cumulative usage（input/output 對應 100 然後 250）經 `Cumulative` 分派後 acc == 250（非 350）；餵一筆 claude `result` 經 `Delta` 分派後維持正確；既有 `accumulate_*` 測試全綠不退步；`CodexBackend::token_usage_semantics()` == `Cumulative`、`ClaudeBackend` == `Delta`。

**Part B — observable behavior**：goal/query/fix/quiz 的 claude spawn argv 含 `--no-session-persistence`；chat 的 claude spawn argv **不含**該旗標且 `--resume <id>` 仍照常出現。
- Interface：`compose_claude_cmd` 新增布林參數（是否加旗標）；`ClaudeBackend::build_command` 以 `!matches!(spec.verb, Verb::Chat)` 推導。
- Acceptance：新單元測試 —— `build_command` 對 `Verb::Goal/Query/Fix/Quiz` 產生的 argv 含 `--no-session-persistence`；對 `Verb::Chat` 不含；`Verb::Chat` + `resume_session_id: Some(id)` 仍含 `--resume id`。

**Part C — observable behavior**：作用中 profile 任一 verb 帶非閉集 effort（如 `ultra` 或 `auto`）→ config 載入回 `ConfigLoadError::YamlParse`、錯誤訊息標明欄位與允許集 `low/medium/high/xhigh/max`；5 個合法值載入成功；非作用中 profile 的非法 effort **不**阻擋載入。GUI 端 dropdown 只剩 5 個選項（無 `auto`）。effort 校驗與 `model` 處理正交 —— 不依 model 值改變行為（model 維持現有處理，本 change 不碰）。
- Interface（Rust）：`CLAUDE_EFFORTS`（5 值）、`validate_effort(effort, field) -> Result<(), ConfigLoadError>`。Interface（GUI）：`SYSTEM_EFFORTS`（5 值、無 `auto`）、`validateClaudeCodeBlock`（連帶只認 5 值）。
- Acceptance：Rust 新單元測試（`codebus-core/tests/endpoint_config_load.rs` 或 endpoint.rs `#[cfg(test)]`）—— active=system 帶 `effort: ultra` 拒載；active=system 帶 `effort: auto` 拒載；active=azure 帶非法 effort 拒載；5 合法值（low/medium/high/xhigh/max）各通過；active=system 而 cold-storage azure 帶非法 effort → 載入成功；effort 校驗不觸碰 model 既有處理（測試用既有合法 model 形式如 `opus-4-6`，**不**新增任何「任意 model 字串通過」斷言——`model: gpt-4` 拒載與否屬既有 `SystemModel` spec/code drift，本 change 不選邊）。GUI 測試：`ipc.effort.test.ts` 斷言 `SYSTEM_EFFORTS` 為 5 值且不含 `auto`、`validateClaudeCodeBlock` 對 `auto` 報 invalid。

**Scope boundaries**：
- In scope：`backend.rs`/`claude_backend.rs`/`codex_backend.rs`/`claude_cli.rs`/`log/sink.rs`/`log/mod.rs`/`config/endpoint.rs` 及其單元測試；**Part C GUI 同步 `codebus-app/src/lib/ipc.ts`（`SYSTEM_EFFORTS` 5 值 + 修註解 + `validateClaudeCodeBlock`）與 `ipc.effort.test.ts`**；三個 spec capability 的 delta（`agent-backend`、`claude-code-config`、`chat-verb`）。
- 刻意不寫 delta：`run-log`（`tokens` 欄位語意「run 總量」不變、Usage 組合機制權威改歸 `agent-backend`，不碰約 180 行的巨型 RunLog Schema requirement，避免 partial-copy 漏 scenario 風險）；`cli`（各 verb argv scenario 用「SHALL include」非窮舉措辭、新 flag 不與其矛盾、argv 權威歸 `agent-backend`）；`app-shell`（「Settings UI Endpoint Section」requirement 未列舉 effort 具體值清單，dropdown 值來自 code 的 `SYSTEM_EFFORTS`，故 5↔6 變動不與其 spec 文字矛盾、不需 delta）。
- Out of scope：codex effort、model 閉集化、cold-storage profile 校驗、`SystemModel` spec/code drift、events.jsonl/runs.jsonl 序列化形狀、mid-flight token 進度 UI（另層 backlog）。`auto` 不走 sentinel/特判。

## Risks / Trade-offs

- [Part A 仍是 latent、非 live] → 既有 smoke 僅見單次 exec 一筆 `turn.completed`（sum==last，今日未復現多計）。Mitigation：負向測試（250 不是 350）守住 by-construction 正確性，與 live/latent 無關；apply 可選擇性復現「單次 exec 是否 emit >1 筆」再定論，但不阻擋本 change。
- [last-wins 覆寫遺失「曾出現過但最後一筆缺漏的欄位」] → 對 cumulative 單調來源不會發生（後筆含前筆）。Mitigation：語意限定 `Cumulative` 專用；`Delta`（claude/預設）行為完全不變。
- [Part C 與前端校驗範圍不一致（core 只校 active、前端校兩 profile）] → 刻意決定。Mitigation：design 明載理由（避免 cold-storage legacy 值阻擋載入）；前端 Save 時的 both-profile 校驗仍是 UI 層 defense-in-depth。
- [新 argv 旗標破壞 `agent-backend`「Argv byte-equivalent to pre-refactor builder」scenario] → 旗標為刻意加法。Mitigation：同 change 校正該 scenario，標明 non-chat verb argv 較 pre-refactor builder 多 `--no-session-persistence`。
- [既有 config 帶 `effort: auto` 改後 load 即被拒] → 行為改變但屬**改善**：今天 `auto` 會 load 過、spawn 才失敗（`--effort auto` 被 CLI 拒）；改後在 load 階段就得到提早、可讀的 `ConfigLoadError`，走既有 config 載入失敗 fallback。Mitigation：proposal Migration 段誠實標明；GUI 移除 `auto` 後使用者下次開 Settings 重選；不做 on-disk 自動改寫（沿用「legacy schema warning without on-disk rewrite」原則）。
