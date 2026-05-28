## Context

兩個 backend 小議題綁同 change：

1. **Codex hosted web search 未關**（Bug 4）：Codex provider 預設啟用 hosted `web_search` tool，允許 agent 連外。codebus 對 agent 設下「offline / sandbox-bounded」契約，但 codex 端 isolation flag set 漏關 `web_search`。`docs/2026-05-28-codex-hook-hard-gate-spike.md` 的 E11 已驗 `-c web_search=disabled` 為正確修法（fix 後 codex 回 `Web search is unavailable.`、不會嘗試對外 fetch）。

2. **`active_runs` key 同秒碰撞**：`spawn_goal` 與 `spawn_chat_turn` 用 `chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)` 派生 RunId（如 `2026-05-28T07-39-26Z`）。`active_runs` 是 `Mutex<HashMap<String, ActiveRunEntry>>`、key 為 RunId；同秒兩次 spawn 會產生相同 key、後者覆蓋前者的 cancel handle、先 spawn 的 run 失去 cancel 通道。2026-05-21 `quiz-double-spawn-guard` 已把 `codebus-app/src-tauri/src/ipc/quiz.rs:120` 改 `Millis`、是本 change 對齊的 reference impl。Bug 3 cross-vault 允許同時兩 goal 跨 vault path spawn 後、碰撞機率提升。

### Pre-apply 校準（grep + Read 結論）

apply 前已用 grep 校準幾個關鍵點，預先在此記錄、避免 apply agent 誤動：

1. **`codex_backend.rs` isolation flag builder 真實位置**：line 117-126（不是 prompt 寫的 118-124）。
   - 現有 flag chain：`--json` / `--ignore-user-config` / `--disable apps` / `--ignore-rules` / `--skip-git-repo-check` / `-c project_root_markers=...` / `-c windows.sandbox=unelevated`（含 2 個 `-c` pair）。
   - 加 `-c web_search=disabled` 進此同 chain、建議放在 `windows.sandbox=unelevated` 之後（同為 `-c` config 覆寫、群聚一處）。

2. **`isolation_flags_always_present` test 真實位置**：codex_backend.rs:401-413，目前 assert 4 個 flag。
   - 加一行 `assert_pair_present(&args, "-c", "web_search=disabled");` 對齊既有 helper（line 386-398 已定義）。

3. **RunId 生成 sites**：`codebus-app/src-tauri/src/ipc/goals.rs:230` + `codebus-app/src-tauri/src/ipc/chats.rs:174`、確認都是 `SecondsFormat::Secs` → 改 `Millis`。`codebus-app/src-tauri/src/ipc/quiz.rs:120` 已 `Millis`、不動。

4. **`events-log/spec.md:62` normative `SecondsFormat::Secs`**：此 spec 強制 verb function 寫 `EventEnvelope.ts` 用 `Secs`。本 change 改的是 IPC 層 RunId 派生、與 events `ts` 是不同 timestamp 來源。**不要動 `codebus-core/src/verb/{chat,goal,query,fix,quiz}.rs` 或 `codebus-core/src/agent/claude_cli.rs` 的 `SecondsFormat::Secs`**——這些是 events log persistence timestamp、本 change scope 外。

5. **`app-workspace/spec.md` 兩 scenario 字面例子**：line 800-803（`spawn_goal returns run id derived from started_at`）字面寫 `"2026-05-13T14-56-21Z"`；line 1385-1388（`spawn_chat_turn returns chat run id`）字面寫 `"chat-2026-05-14T10-20-30Z"`。改 Millis 後例子要更新成像 `"2026-05-13T14-56-21.123Z"` / `"chat-2026-05-14T10-20-30.456Z"`、否則 spec example 與實際 IPC 回傳 mismatch。

6. **`codex-backend/spec.md` line 11 + 30 兩處 isolation flag enumeration**：line 11 在 requirement body、line 30 在 scenario「**THEN** the composed argv SHALL contain all of `--ignore-user-config`, `--disable apps`, `--ignore-rules`, ...」。兩處都要加 `web_search=disabled` 對齊實作。

## Goals / Non-Goals

**Goals:**

- Codex provider 端 hosted `web_search` 永久關閉、agent 無法對外 fetch；regression test 防回流。
- `spawn_goal` / `spawn_chat_turn` 的 `active_runs` key 同秒不碰撞；regression test 覆蓋同 ms / cross-vault 場景。
- Spec 兩處 scenario 字面例子與實際 IPC 回傳對齊。

**Non-Goals:**

- 不動 `--disable image_generation`（user 決議保留）。
- 不動 `codebus-core/src/verb/*` 與 `codebus-core/src/agent/claude_cli.rs` 內 `SecondsFormat::Secs`（events-log normative + 不在本 scope）。
- 不動 `codebus-app/src-tauri/src/ipc/quiz.rs`（已是 Millis）。
- 不動 frontend / i18n / AUDIT.md。
- 不分兩個 change（user 已 bundle）。

## Decisions

### Decision 1 — Codex web_search 用 `-c web_search=disabled` 而非 `--disable web_search`

`docs/2026-05-28-codex-hook-hard-gate-spike.md` E11 驗證過：codex 的 `--disable` flag 限定 `apps` / `image_generation` 等內建 sub-feature ID，hosted web search 不在 `--disable` 接受清單。`-c web_search=disabled` 走 config override 路徑、生效。Spike doc 顯示 fix 後 codex 回 `Web search is unavailable.`、行為符合預期。

**Alternative 考慮**：
- `--disable web_search`：codex CLI 不接受、會回 unknown sub-feature 錯誤、放棄。
- Network sandbox 全鎖：太重、會破壞 codex 對 OpenAI / Azure endpoint 的正常呼叫、放棄。

### Decision 2 — `SecondsFormat::Millis` 對齊 quiz.rs reference

`codebus-app/src-tauri/src/ipc/quiz.rs:120` 已用 `Millis`、含 inline 註解「millisecond precision so two spawns within the same wall-clock second receive distinct ids」。goals.rs / chats.rs 對齊同精度、註解可複用 quiz.rs 的文字模板（簡化為單行）。

**Alternative 考慮**：
- `Nanos` 精度：過剩、Windows 時鐘解析度通常 ms 或 100ns、Nanos 純視覺噪音、放棄。
- 用 UUID v7 或 monotonic counter：偏離既有 codebase pattern（quiz 已示範 Millis 解法）、引入新 dep、放棄。

### Decision 3 — Spec scenario 例子改 Millis、補 requirement 字面規範精度

`app-workspace/spec.md` 兩 scenario 字面例子目前用 Secs。本 change 把例子改 Millis（`2026-05-13T14-56-21.123Z` / `chat-2026-05-14T10-20-30.456Z`），並在對應 requirement body 補一句「`RunId` SHALL be derived from `chrono::Utc::now()` using `SecondsFormat::Millis` so two spawns within the same wall-clock second yield distinct keys」。Spec layer 對精度做 minimum 規範、避免未來再回 Secs 而 collision 重現。

### Decision 4 — Regression test 落點：抽 helper 函式 + 同檔 unit test

兩個選項：
- (a) `active_runs.rs` 內測試（已有 `mod tests`、line 129+）：覆蓋 collision-safety 屬性、但 RunId 派生不在 `ActiveRuns` 模組、test 要 mock。
- (b) `goals.rs` / `chats.rs` 內測試：直接呼叫 `spawn_goal` / `spawn_chat_turn` 同 ms 連續兩次（要 mock Tauri AppHandle / runtime）。

**選擇 (a) 變體**：把 RunId 派生抽成 `fn goal_run_id() -> String` / `fn chat_run_id() -> String` 小函式（goals.rs / chats.rs 內、`pub(crate)` visibility）、test 連續呼叫兩次驗證不重複。`active_runs.rs` 不擴 scope。

**Alternative 考慮**：
- 完整 spawn flow test：要 mock Tauri AppHandle / runtime、ceremony 過重、cost 大於 benefit、放棄。

## Implementation Contract

### Observable behavior

- **Codex web search**：codex provider 跑任何 verb（goal / chat / query / fix / quiz）時，hosted web search tool 不可用；agent 若嘗試 web search、codex 回 `Web search is unavailable.` 或同等 fallback、不會對外 fetch URL。
- **RunId collision**：同一 process 內、同一 wall-clock millisecond 邊界、連續兩次呼叫 `spawn_goal` 或 `spawn_chat_turn`（跨 vault path）SHALL 得到兩個不同 RunId、`active_runs` HashMap 同時存有兩 entry、兩 entry 的 cancel handle 互不覆蓋。

### Interface / data shape

- `CodexBackend::build_command`：argv 額外含 `-c` `web_search=disabled` pair；其餘 argv 順序不變。
- `RunId` 字面格式從 `YYYY-MM-DDTHH-MM-SSZ`（goal）/ `chat-YYYY-MM-DDTHH-MM-SSZ`（chat）變成 `YYYY-MM-DDTHH-MM-SS.fffZ` / `chat-YYYY-MM-DDTHH-MM-SS.fffZ`。字串長度增加 4 個 char（`.fff`）。`active_runs` HashMap key type 不變（`String`）、`runs-*.jsonl` schema 不變、frontend 不需特殊解析（RunId 視為 opaque string）。

### Failure modes

- **Codex 端不支援 `web_search` config key**（極端）：codex 會忽略未知 config key、isolation 不被破壞、test 通過、warning 不噴。Spike 已驗 codex 接受此 key。
- **RunId millis 仍可能碰撞**（理論）：同一 ms 邊界內仍有可能、但 Windows 時鐘 100ns 級解析度下機率近 0。完整防護需 monotonic counter、不在本 scope。

### Acceptance criteria

1. `cargo test -p codebus-core agent::codex_backend` 綠：
   - `isolation_flags_always_present` 含 `web_search=disabled` assertion。
2. `cargo test -p codebus-app-tauri` 綠：
   - 新測試「goal RunId 同 ms 兩次不重複」綠。
   - 新測試「chat RunId 同 ms 兩次不重複」綠。
3. `pnpm tsc --noEmit` + `pnpm test` 綠（frontend 不應動到、RunId 多 4 字也不影響 string compare）。
4. **真實 CDP smoke**（per `project_cdp_smoke_webview2_pitfalls`）：
   - 開 codex provider 跑 goal、prompt 含 web search 觸發詞（如 `"what's the latest news on X"`）、codex 回 `Web search is unavailable.` 或同等 fallback、不對外 fetch。
   - 兩個 vault A / B、快速 alt-click「Run」兩個 goal、兩個 run 都成功 spawn、`active_runs` 同時兩 entry、cancel A 不影響 B。
   - 截圖存 `codebus-app/scripts/.backend-cleanup-smoke/`。

### Scope boundaries

**In scope:**
- `codebus-core/src/agent/codex_backend.rs`：argv builder + test。
- `codebus-app/src-tauri/src/ipc/goals.rs`：RunId 派生（line 230 區）。
- `codebus-app/src-tauri/src/ipc/chats.rs`：RunId 派生（line 174 區）。
- `openspec/specs/codex-backend/spec.md`：requirement 字面 + scenario assertion。
- `openspec/specs/app-workspace/spec.md`：兩 scenario 字面例子 + 補述 `SecondsFormat::Millis` 規範。

**Out of scope:**
- `codebus-core/src/verb/*` + `codebus-core/src/agent/claude_cli.rs` 的 `SecondsFormat::Secs`（events-log normative）。
- `codebus-app/src-tauri/src/ipc/quiz.rs`（已是 Millis）。
- `--disable image_generation`（user 決議保留）。
- frontend / i18n / AUDIT.md。

## Risks / Trade-offs

- **[Risk] RunId 字面長度增加可能 break frontend hard-code 解析** → Mitigation：apply 階段 grep `T..-..-..Z` 或 `chat-2026` 字面確認 frontend 無 hard-code 解析、grep 結論前端把 RunId 當 opaque string。
- **[Risk] Spec scenario 例子變動可能影響其他 reference** → Mitigation：grep 確認兩 scenario 內字面 timestamp 僅該 scenario 自身引用、改完跑 `spectra validate`。
- **[Risk] Codex CLI 版本升級可能改 `web_search` config key 名稱** → Mitigation：test assertion 覆蓋、版本升級時 test 會先 break、不會悄悄回流。
- **[Trade-off] 不引入 monotonic counter / UUID v7**：Millis 已解 99.99%+ 場景、剩餘理論 collision 不值得引入新 dep。
