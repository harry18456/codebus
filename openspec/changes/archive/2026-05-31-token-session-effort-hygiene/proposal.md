## Summary

補三件 provider backend / config 的 hygiene 與潛伏 correctness：codex token 累加改 cumulative-replace（潛伏 double-count 防護）、claude 非 chat verb 加 `--no-session-persistence`（orphan session 衛生）、Rust loader 補 claude effort 閉集校驗（fail-late → fail-early hardening）。三件都小且彼此獨立，共同主題是「provider spawn / 結果解析 / config 的收尾正確性」。

## Motivation

三個既有缺口各自獨立、單獨開 change 都過小，合併為一個 cohesive 的 hygiene change：

- **Part A（潛伏 double-count）**：`agent::invoke` 對每個 `StreamEvent::Usage` 一律呼叫 `accumulate_token_usage`（欄位 `saturating_add` 求和）。claude 的 `result.usage` 一個 `-p` run 只 emit 一筆，求和無誤；但 codex 的 `turn.completed.usage` 帶的是**累計總量（cumulative-replace），非 per-turn delta**。一旦單一 codex spawn emit 一筆以上 `turn.completed`，求和即 double-count。**這是 latent（潛伏）而非 live bug**——既有 smoke 觀測單次 codex `exec` 只 emit 一筆 `turn.completed`（sum == last），尚未復現實際多計；但求和語意 by-construction 對 cumulative 來源是錯的，值得在它咬人前先 harden。

- **Part B（orphan session 衛生）**：codex backend 已對非 `Chat` verb 加 `--ephemeral`（不留 session rollout 檔），唯獨 claude backend 的 `compose_claude_cmd` 沒有對應的 `--no-session-persistence`。goal / query / fix / quiz 從不 resume，卻每次都讓 claude CLI 落地一份 session 檔，累積成 orphan session（磁碟 + 隱私殘留）。`--no-session-persistence` 只在 `-p`（print）模式有效，而 codebus 正是用 `-p`。

- **Part C（fail-late hardening + 修錯閉集）**：`SystemVerbConfig.effort` / `AzureVerbConfig.effort` 在 Rust loader 是無校驗的 `String`（註解寫「交給 Claude CLI 驗」）。非法 effort（如 `ultra`）能載入成功、一路傳到 spawn 才由 CLI 報錯（fail-late）。把閉集下移到 core loader，讓非法值在載入階段就被 `ConfigLoadError` 擋下。**閉集 = 5 值 `low/medium/high/xhigh/max`**——已用 `claude --help` 實證 `--effort <level>` 只接受這 5 值。前端 `ipc.ts` 的 `SYSTEM_EFFORTS` 目前含 `auto`（6 值）且上方註解宣稱「mirrors … --effort accepted set (… / auto)」**是錯的**：CLI 不收 `auto`，而 `compose_claude_cmd` 把 effort 原樣丟 `--effort`、沒特判，使用者在 GUI 選 `auto` → 送 `--effort auto` → CLI 拒 → **spawn 失敗**（`auto` 今天就是壞的、從未能用）。本 part 同步把 GUI 的 `SYSTEM_EFFORTS` 砍成 5 值並修正錯註解，讓 GUI / loader / CLI 三方一致。

## Proposed Solution

- **Part A**：在 `AgentBackend` trait 加一個 opt-in 預設方法 `token_usage_semantics() -> TokenUsageSemantics`（預設 `Delta`），codex backend override 為 `Cumulative`。`invoke` 在迴圈外向 backend 取一次語意，迴圈內改呼叫一個 provider-neutral 的 dispatch 函式：`Delta` → 既有求和（`accumulate_token_usage`）；`Cumulative` → 以最後一筆覆寫（last-wins）。`invoke` 仍對 provider 完全無感（只 match 語意 enum、不出現任何 provider 名），符合 `agent-backend` 既有 invariant「loop 內不得引用 provider 名 / argv flag / stream-json 欄位」。`StreamEvent::Usage` 的序列化形狀、`TokenUsage` 正規化 DTO、events.jsonl / runs.jsonl 格式皆不動。

- **Part B**：`ClaudeBackend::build_command`（持有 `spec.verb`）計算 `!matches!(spec.verb, Verb::Chat)`，把布林 thread 進 `compose_claude_cmd`，非 chat verb 時加 `--no-session-persistence`。chat 維持持久化讓 `--resume` 可用（鏡像 codex 的 `Verb::Chat` gate）。

- **Part C**：在 `endpoint.rs` 新增共用 `validate_effort` helper（閉集 = 5 值 `low/medium/high/xhigh/max`），於 `validate_system_profile` / `validate_azure_profile` 的**作用中（active）profile** 路徑對 goal / query / fix / verify 四個 verb 的 effort 做閉集校驗，非法值（含 `auto`）回 `ConfigLoadError::YamlParse`。**不走 backend sentinel／不特判 `auto`→omit**——`auto` 本就不該存在。GUI 同步：`ipc.ts` 的 `SYSTEM_EFFORTS` 移除 `auto`（剩 5 值）+ 修正其上方錯註解（改為「CLI accepts low/medium/high/xhigh/max」、拿掉 auto sentinel 描述）；`validateClaudeCodeBlock` 因讀 `SYSTEM_EFFORTS` 自動只認 5 值。**只校 claude；model 維持寬鬆 forward-compat 不動。** 非作用中（cold-storage）profile 依既有原則不校驗。

## Non-Goals

- **不閉集化 model**：`model` 維持自由字串 + `system_model_to_cli_flag` 的 `claude-` 前綴 forward-compat（新 Claude 模型免改碼）。
- **不動 codex effort**：`codex.rs` 的 effort 全集文件未列全、閉集化證據不足，本 change 不碰。
- **不校驗 cold-storage（非作用中）profile 的 effort**：遵循 `claude-code-config` 既有原則「codebus SHALL NOT validate fields of the non-active profile」。前端 `validateClaudeCodeBlock` 在 Save 時校兩個 profile 是 UI 層 defense-in-depth；core loader 只對實際會送進 `--effort` 的 active profile 把關。此為刻意與前端不同範圍的決定（見 design.md）。
- **不在本 change 跑真 codex 多步 exec 來把 Part A 從 latent 升級為 live**：propose 階段以既有 smoke 證據誠實標為 latent；apply 階段可選擇性復現「單次 exec 是否 emit >1 筆 turn.completed」再定論。負向測試（250 不是 350）無論 live/latent 都成立。

## Alternatives Considered

- **Part A：在 parser 標記語意並夾帶於事件中**（把 `cumulative` flag 放進 `StreamEvent::Usage` 或 `TokenUsage`）。否決：`StreamEvent` 經 `VerbEvent::Stream` 持久化到 events.jsonl、`TokenUsage` 是寫進 runs.jsonl 的正規化 DTO；改其序列化形狀會破壞既有 jsonl 格式相容性，且把一個只有 `invoke` 用得到的 transient 收尾語意污染進持久化 schema。改用 backend trait 的 opt-in 方法（與既有 `stdin_payload` 同模式），語意留在 backend（provider 知識的歸屬地），`invoke` 以多型分派保持中立，零持久化形狀變動。
- **Part A：直接在 invoke 裡 `if codex { replace } else { sum }`**。否決：違反 `agent-backend` 的 provider-agnostic invariant。

## Migration（Part C effort 閉集）

既有 `~/.codebus/config.yaml` 若帶 `effort: auto`：**今天**的行為是「load 成功、spawn 才爛」（`--effort auto` 被 CLI 拒、該 verb spawn 失敗）；**改後**的行為是「load 階段即被 `ConfigLoadError` 拒、走既有 config 載入失敗的 fallback 路徑」——是**改善**（錯誤更早、更可讀，不再到 spawn 才炸）。GUI 端 `SYSTEM_EFFORTS` 拔掉 `auto` 後，使用者下次開 Settings 會看到只剩 5 個選項、需重選一個合法 effort。本 change 不做 on-disk 自動改寫（沿用既有「legacy schema warning without on-disk rewrite」原則）。

## Impact

- Affected specs:
  - `agent-backend`（修改，有 delta）— ADDED「Provider-Declared Token Usage Semantics」（trait 方法 + invoke 依語意分派，含 `RunLog.tokens` 反映組合結果之權威陳述）；MODIFIED「Claude Backend Argv Equivalence」（加 claude session 持久化 gating：goal/query/fix/quiz 含 `--no-session-persistence`、chat 不含；校正 byte-equivalence scenario）。
  - `claude-code-config`（修改，有 delta）— MODIFIED「Endpoint Profile Schema」：effort 由「arbitrary string」改為作用中 profile 的**五值**閉集 `low/medium/high/xhigh/max`（`auto` 不合法）；新增校驗 scenarios（含「effort: auto 被拒」）。
  - `chat-verb`（修改，有 delta）— ADDED「Chat Session Persistence Retained」：chat spawn 保留 session 持久化（不含 `--no-session-persistence`），`--resume` 前提成立。
  - `run-log`（無 delta）— `RunLog.tokens` 欄位語意「該 run 的 token 總量」不變（codex cumulative 最後一筆即總量）；Usage 事件如何組合（sum vs last）的權威機制改由 `agent-backend` 新需求定義，故巨型「RunLog Schema」requirement 不動。
  - `cli`（無 delta）— goal/query/fix 各 verb spawn argv scenario 使用「SHALL **include**」非窮舉措辭，新增 `--no-session-persistence` 不與其矛盾；argv 權威歸 `agent-backend`。
- Affected code:
  - Modified:
    - codebus-core/src/agent/backend.rs
    - codebus-core/src/agent/claude_backend.rs
    - codebus-core/src/agent/codex_backend.rs
    - codebus-core/src/agent/claude_cli.rs
    - codebus-core/src/log/sink.rs
    - codebus-core/src/log/mod.rs
    - codebus-core/src/config/endpoint.rs
    - codebus-app/src/lib/ipc.ts （Part C GUI 同步：`SYSTEM_EFFORTS` 移除 `auto` → 5 值、修正其上方錯註解；`validateClaudeCodeBlock` 連帶只認 5 值）
    - codebus-app/src/lib/ipc.effort.test.ts （更新測試：six→five、移除 auto 斷言）
  - New: (none)
  - Removed: (none)
