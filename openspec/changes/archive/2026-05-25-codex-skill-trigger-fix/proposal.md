## Why

2026-05-24 實機跑 5 verb × codex provider 矩陣，codex 0.133.0 上 codebus 的 SKILL invocation 失靈：claude path 5/5 work、codex path 0/5 完整 work。完整 reproducer + 對照已記錄於 `docs/2026-05-24-codex-provider-experiment.md` 與 memory `todo_codex_provider_regression_2026-05-25`，2026-05-25 重跑 reproducer 確認問題仍在。

multi-provider 框架的核心承諾就是「換 provider 不換語意」。codex provider 是當前唯二的 driver（per memory `project_multi_provider_driver_confirmed`），整段 codex provider 不可用就讓 multi-provider extensibility 設計失去 second-impl 驗證。

2026-05-25 diagnose 揭示 **兩個獨立 bug 都不在原 propose 時推測的位置**，且兩者都坐落於 `codebus-core/src/vault/init.rs` + `codebus-core/src/agent/codex_backend.rs::build_command` 的 isolation recipe 層、修法形狀是同函數內的一行改動 — 因此原本拆 A/B 兩 change 的方案改為一個 change 一併處理（詳 `docs/2026-05-25-codex-skill-trigger-diagnose.md`）。

## What Changes

1. **Diagnose 三層**（依序執行，找到根因後停止）：
   - (a) codex CLI 版本對照：在 isolated 環境安裝 codex 0.132.0 跑同 reproducer 判定是否為 codex 0.133.0 regression。
   - (b) argv 攔截：透過 `CODEBUS_CODEX_BIN` 環境變數指向 shim binary，dump codebus 實際送給 codex 的 argv + prompt 字串，驗證 sigil 與 isolation flags 完整性。
   - (c) codex 端行為觀察：直接呼叫 `codex exec ...` 看 SKILL 識別 / sandbox 寫權限是否如 codebus 預期。
2. **A cluster 修法（SKILL trigger）— vault init codex materialization 改無條件**：`codebus-core/src/vault/init.rs::init_vault` 中對 codex SKILL bundles + `.codebus-vault` marker + `AGENTS.md` 的 materialization 從「if `active_provider == codex`」改為無條件 write-if-missing。原 gate 設計（避免 claude-only vault 被 codex 材料污染）失效於「用戶 init 後切 provider」場景，新設計兩 provider 材料共存、各自 backend 不互讀。
3. **B cluster 修法（codex sandbox-write 實際可寫）— isolation recipe 補 `-c windows.sandbox=unelevated`**：`codebus-core/src/agent/codex_backend.rs::build_command` 在原本的 `--ignore-user-config + --disable apps + --ignore-rules + -s workspace-write` recipe 之上補一條 `-c windows.sandbox=unelevated`。本 override 抵銷 `--ignore-user-config` 對 Windows sandbox 寫權限預設值的副作用（user config 本身有此 key 啟用、被 `--ignore-user-config` 拿掉後 sandbox 退回 read-only，即使 CLI flag `-s workspace-write` 也 honor 不到）。保留 `--ignore-user-config` 仍能擋掉 MCP / plugin / personality / trust-list 等不要的滲入。
3.5. **C cluster 修法（codex verify-stage spawn 不再 batch-file argv 失敗）— 多行 prompt 走 stdin**：Rust 1.77+ stdlib 對 `.cmd` / `.bat` 執行檔的 argv 加了驗證，含 `\n` 的 arg 會以 `InvalidInput: batch file arguments are invalid` 拒絕。codex npm 安裝在 Windows 上是 `codex.cmd` shim。本 change 在 `codebus-core/src/agent/codex_backend.rs::build_command` 對多行 prompt（verify / repair sub_modes 把 CHANGED PAGES / CONTENT DEFECTS 等用 `\n` 連起來）改用 `-` 當 prompt argv、把 formatted prompt 透過 stdin pipe 餵 codex；單行 prompt 維持原 argv 形式。實作 `AgentBackend::stdin_payload()` 新增的 optional method（default `None`、claude 不動）。修完後 quiz / goal 的 content-verify stage 不再印「`warning: ... content-verify spawn failed: batch file arguments are invalid`」、verify 真實跑 + 回 `CONTENT_OK` 或具體 defect lines。
4. **記錄 diagnose 結果**：`docs/2026-05-25-codex-skill-trigger-diagnose.md` 收斂三層觀察與結論、含 K 模式 bisect 證據（codex tool 列表確認 node_repl MCP 沒載、workspace-write 真的生效），便於日後 codex 版本 bump 時回溯。
5. **5 verb × codex 重跑驗證**：goal/query/fix/chat/quiz 全用 codex provider 重跑 `/tmp/exp-vault`，行為對等 claude path（quiz 三 stage、goal 寫 wiki page、query 引 wikilinks、chat 不 emit meta-comment、fix 真的 edit 修 lint warning）。

## Non-Goals (optional)

- **不動 claude path**：claude 5/5 work，不重構共用層，不在本 change 引入 claude 端可觀察的 spec 變更。
- **不重構 multi-provider 抽象層**：本 change 是 codex 端 isolation recipe 補漏、不是 driver 抽象重新設計；不新增 trait method、不新增 SpawnSpec 欄位、不新增 strategy enum。
- **不另開 inventory finding**：本 change 是 codex backend regression、不在 prompt-surface review 範圍（per memory `todo_prompt_surface_review_phase_0` 已 archive）。
- **不延伸到 Phase 5 spike**：codex per-command allowance 探索（P3 in memory todo）走不到 Mode B，本 change 不處理；待本 change 修完後另議。
- **macOS / Linux sandbox-write 不在本 change 驗證範圍**：`-c windows.sandbox=unelevated` 是 Windows-only key（codex `[windows]` table）；macOS（seatbelt）/ Linux（Landlock）各有原生 sandbox 後端，需各自驗證對等 override 是否需要。本 change 在 Windows 上驗 5/5 verb work、spec 明示其他平台行為 deferred、不視為 ship-blocker（per memory `feedback_dont_default_polish_ship`，solo dev + 主要在 Windows 開發測試）。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `skill-bundles`: SKILL bundle 在 codex provider 上 SHALL 觸發 SKILL Mode invocation，使 agent 進入 verb-specific workflow 而非 generic task-reply mode；前提是 vault init 已 materialize codex 端材料（無條件 init 已保證）。
- `codex-backend`: isolation recipe SHALL 包含一條 Windows-specific sandbox elevation override，使 `-s workspace-write` 在 `--ignore-user-config` 啟用時仍然能讓 codex agent 對 vault workdir 真實可寫；多行 prompt SHALL 走 child process stdin（用 `-` 當 prompt arg）避開 Rust 1.77+ `.cmd` shim 的 batch-file argv 驗證。
- `agent-backend`: AgentBackend trait 從「exactly three methods」鬆綁為「3 個 required + optional methods with safe defaults」，新增 optional `stdin_payload(&SpawnSpec) -> Option<String>`（default `None`、claude 走預設、codex 在 prompt 多行時 opt-in）。

## Impact

- Affected specs:
  - `openspec/specs/skill-bundles/spec.md`（ADDED「Codex-Side SKILL Mode Invocation Trigger」requirement，含 5 verb + failure-surfacing scenario，fix scenario 不再 carve-out sandbox refusal）
  - `openspec/specs/codex-backend/spec.md`（ADDED「Codex Sandbox Write Enablement Override」+「Codex Multi-Line Prompt Stdin Routing」requirements）
  - `openspec/specs/agent-backend/spec.md`（MODIFIED「Agent Backend Trait Contract」requirement — 鬆綁 exactly-three、加 optional `stdin_payload` method scenario）
- Affected code:
  - Modified:
    - `codebus-core/src/vault/init.rs::init_vault`（解除 `codex_provider_active()` gate）
    - `codebus-core/src/agent/codex_backend.rs::build_command`（isolation recipe 補 `-c windows.sandbox=unelevated`；多行 prompt 走 `-` + `stdin_payload`）
    - `codebus-core/src/agent/codex_backend.rs`（implement `AgentBackend::stdin_payload`，加 `format_codex_prompt` helper）
    - `codebus-core/src/agent/backend.rs`（trait 新增 default `stdin_payload(&SpawnSpec) -> Option<String>` returning `None`）
    - `codebus-core/src/agent/claude_cli.rs::invoke`（stdin Stdio::piped + write payload when backend opt-in）
  - New:
    - `docs/2026-05-25-codex-skill-trigger-diagnose.md`（diagnose 三層 + K 模式 bisect + C cluster argv 對比 + 選用修法理由）
  - Removed:
    - `codebus-core/src/vault/init.rs::codex_provider_active`（gate 解除後成 dead code）
- 對 claude path 無預期影響（init.rs 變動只多 unconditional write codex 材料、claude backend 不讀；trait 新增 default `stdin_payload=None` 不影響 claude 行為；invoke 改用 conditional stdin pipe、claude path 仍走 Stdio::null()）。
- 對既存 codex-backend / codex-config archive 的影響：本 change ADDED 兩條新 requirement、不 retract 既有 argv composition / Permission mapping / Azure routing requirement。
- 對 agent-backend archive 的影響：本 change MODIFIED「Agent Backend Trait Contract」requirement — 從「exactly three methods」鬆綁為「3 required + optional methods with safe defaults」；既有實作（claude）不受影響，新 optional method 對 claude 是 default `None`。
