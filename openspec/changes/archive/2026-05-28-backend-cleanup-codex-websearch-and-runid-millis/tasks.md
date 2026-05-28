## 1. Pre-apply 校準（grep + Read 結論）

- [x] 1.1 重跑 grep `SecondsFormat::Secs` 與 `ignore-user-config` 校準 ground truth（per design「Pre-apply 校準（grep + Read 結論）」段）：確認 `codebus-app/src-tauri/src/ipc/goals.rs:230`、`codebus-app/src-tauri/src/ipc/chats.rs:174` 仍是 `Secs`、`codebus-app/src-tauri/src/ipc/quiz.rs:120` 仍是 `Millis`、`codebus-core/src/agent/codex_backend.rs` line 117-126 仍是 isolation flag builder、line 401-413 仍是 `isolation_flags_always_present` test。驗證：grep 結果與 design 文字一致；若任何 line 號或檔名漂移、在 tasks.md 同節記錄並把後續任務改用 symbol（`build_command` / `isolation_flags_always_present`）而非 line 號 anchor。

## 2. Codex web_search disabled flag

- [x] 2.1 在 `CodexBackend::build_command` 的 isolation flag chain 加入 `-c web_search=disabled` config override，使 Codex Backend Argv Composition requirement 內「`-c web_search=disabled` config override that turns off codex's hosted web search tool」生效（per design Decision 1 — Codex web_search 用 `-c web_search=disabled` 而非 `--disable web_search`）。驗證：argv 中含 `-c` + `web_search=disabled` 連續 pair；現有 isolation argv（`--ignore-user-config` / `--disable apps` / `--ignore-rules` / `project_root_markers`）順序不破壞。
- [x] 2.2 擴充 `isolation_flags_always_present` test：新增一行 `assert_pair_present(&args, "-c", "web_search=disabled");`，使 Codex Backend Argv Composition 的「Isolation flags always present」scenario 在 unit test 層有 regression 覆蓋（per design Observable behavior 段）。驗證：`cargo test -p codebus-core --lib agent::codex_backend::tests::isolation_flags_always_present` 綠。

## 3. RunId Millis precision for goal and chat spawn

- [x] 3.1 在 `codebus-app/src-tauri/src/ipc/goals.rs` 把 `started_at` 派生從 `SecondsFormat::Secs` 改 `SecondsFormat::Millis`，並抽出 `pub(crate) fn goal_run_id() -> String` helper（per design Decision 2 — `SecondsFormat::Millis` 對齊 quiz.rs reference + Decision 4 — Regression test 落點：抽 helper 函式 + 同檔 unit test）。驗證：`spawn_goal` 回傳之 `RunId` 字面符合 `YYYY-MM-DDTHH-MM-SS.fffZ` 形式；helper 為 `pub(crate)` 可供同檔 test 呼叫；對應 Tauri IPC Commands for Goal Lifecycle and Wiki Read requirement 的新 Millis 規範生效。
- [x] 3.2 在 `codebus-app/src-tauri/src/ipc/chats.rs` 把 `started_at` 派生從 `SecondsFormat::Secs` 改 `SecondsFormat::Millis`，並抽出 `pub(crate) fn chat_run_id() -> String` helper。驗證：`spawn_chat_turn` 回傳之 `RunId` 字面符合 `chat-YYYY-MM-DDTHH-MM-SS.fffZ` 形式；對應 Tauri IPC Commands for Chat Turn Lifecycle requirement 的新 Millis 規範生效。
- [x] 3.3 在 `goals.rs` 與 `chats.rs` 各加 unit test（同檔 `mod tests`）：連續呼叫 helper 兩次（兩呼叫之間如有需要可短暫 `std::thread::sleep(Duration::from_millis(1))` 確保跨 ms 邊界）、assert 兩次回傳字串不相等、assert 字串含 `.` fractional separator（per design Interface / data shape 段）。驗證：`cargo test -p codebus-app-tauri ipc::goals` 與 `cargo test -p codebus-app-tauri ipc::chats` 綠；test 失敗時錯誤訊息能直接定位到 helper 函式。

## 4. Spec scenario examples alignment

- [x] 4.1 確認 capability spec `openspec/specs/codex-backend/spec.md` 與 `openspec/specs/app-workspace/spec.md` 在本 change archive 後會帶入：codex-backend 加入 `-c web_search=disabled` 進 isolation enumeration、app-workspace 兩個 scenario 字面例子改為 `2026-05-13T14-56-21.123Z` / `chat-2026-05-14T10-20-30.456Z` 並補 Millis 規範（per design Decision 3 — Spec scenario 例子改 Millis、補 requirement 字面規範精度 + Scope boundaries 段）。驗證：`spectra validate backend-cleanup-codex-websearch-and-runid-millis` 綠；`spectra analyze backend-cleanup-codex-websearch-and-runid-millis --json` Critical/Warning 為空。

## 5. End-to-end verification

- [x] 5.1 跑 `cargo test -p codebus-core agent::codex_backend` 全綠、跑 `cargo test -p codebus-app-tauri` 全綠（per design Acceptance criteria 段）。驗證：兩個 cargo test 命令 exit 0、含本 change 新增的 3 個 test（`isolation_flags_always_present` 含 web_search assertion、goal_run_id 不重複、chat_run_id 不重複）。
- [x] 5.2 跑 `pnpm tsc --noEmit` 與 `pnpm test`（frontend regression）。驗證：兩命令 exit 0；確認 RunId 字面長度增加 4 字（`.fff`）不破壞 frontend 任何 hard-code 字面解析（per design Failure modes 段對 RunId 字面影響的記錄）。
- [x] 5.3 真實 CDP smoke（per design Acceptance criteria 與 [[project_cdp_smoke_webview2_pitfalls]] 教訓）：開 codex provider 跑 goal、prompt 含 web search 觸發詞（如 `what's the latest news on X`）、觀察 codex 回應「Web search is unavailable.」或同等 fallback、無對外 fetch 行為。驗證：截圖存到 `codebus-app/scripts/.backend-cleanup-smoke/codex-web-search-disabled.png`；agent 回應內容明確指出 web search 不可用。
- [x] 5.4 真實 CDP smoke：跨 vault path（V1 / V2）快速連續 `spawn_goal` 與 `spawn_chat_turn`、確認 `AppState.active_runs` 同時存兩 entry、cancel 其中一個不影響另一個（per design Risks / Trade-offs 段 RunId 字面長度與 collision 緩解）。驗證：截圖存 `codebus-app/scripts/.backend-cleanup-smoke/cross-vault-millis-runid.png`；devtools console 或 IPC log 顯示兩個不同 `RunId` 字串。

## 6. Verb 端 run_started_at 對齊 Millis（apply 階段補 invariant fix）

實機 CDP smoke 抓到 design Pre-apply 校準漏掉的 invariant break：IPC 層 `active_runs` key 改 Millis 後、verb 端 `events-*.jsonl` filename slug 仍是 Secs、`list_runs_impl` 內「Orphan goal events file with live active_runs entry produces running virtual entry」場景下 `active_runs.get(slug)` 永遠 miss、活著的 goal 被誤標 `interrupted`。Decision：採選項 B 根本解（verb 端 `run_started_at` 也改 Millis、events filename slug + RunLog row started_at 與 IPC active_runs key 同精度）、不走 strip-at-lookup symptom patch。

- [x] 6.1 把 `codebus-core/src/verb/goal.rs:140` 的 `run_started_at` 從 `SecondsFormat::Secs` 改 `SecondsFormat::Millis`、events filename slug + RunLog `started_at` 對齊 IPC `active_runs` key 精度。驗證：grep 該 site 出現 `Millis`；同檔 `EventEnvelope.ts`（events-log spec line 62 強制 Secs）不變。
- [x] 6.2 同樣改 `codebus-core/src/verb/chat.rs:123`、`codebus-core/src/verb/quiz.rs:495`（quiz_generate；quiz.rs:425 是 plan step 不寫 events/RunLog、不動）、`codebus-core/src/verb/query.rs:80`、`codebus-core/src/verb/fix.rs:105`。驗證：grep `SecondsFormat::Secs` 在 verb 內剩下的全是 `EventEnvelope.ts` 用途；無 `run_started_at` 派生點仍是 Secs。
- [x] 6.3 新增 unit test 覆蓋 invariant：在 `codebus-app/src-tauri/src/ipc/goals.rs` `mod tests` 加 test 跑 `spawn_goal_with_runner` stub runner 寫 events 後立即 list_runs、assert events file slug 集合與 active_runs key 集合對齊（同精度同 string）。驗證：`cargo test -p codebus-app-tauri ipc::goals` 含此新 test 綠；若 invariant 再 drift 此 test 會直接 break。
- [x] 6.4 補強 spec：在 `openspec/specs/app-workspace/spec.md` Interrupted Run Detection requirement（line 873+）加 NOTE「`active_runs` key 與 events filename slug + RunLog `started_at` SHALL be derived at the SAME `SecondsFormat::Millis` precision; if a future change splits them, the orphan-detection invariant breaks and live goals will be mis-labeled `interrupted`」。更新本 change 的 `specs/app-workspace/spec.md` delta 加入 Interrupted Run Detection 的 MODIFIED entry。驗證：`spectra validate` 綠、`spectra analyze` Critical/Warning 為空。
- [x] 6.5 重跑 `cargo test -p codebus-core` 全綠（5 處 verb 改動）、重跑 `cargo test -p codebus-app-tauri` 全綠（含新 invariant test）、`pnpm tsc --noEmit` + `pnpm test` 綠。驗證：所有命令 exit 0。
- [x] 6.6 真實 CDP smoke 重驗 Task 5.3 / 5.4：spawn 一個 goal、events file 寫進去、goal 還活著時 list_runs 應回 `outcome: "running"`（非 interrupted）；cancel 後再 list_runs 才應回 `cancelled`。驗證：cdp eval 輸出顯示活著的 goal 是 running；截圖存 `codebus-app/scripts/.backend-cleanup-smoke/live-goal-running.png`。
- [x] 6.7 Archive 階段順手：寫 `docs/2026-05-28-runid-source-of-truth-todo.md` 紀錄 latent issue（IPC + verb 兩處獨立派生 timestamp、若未來 drift 仍會 break invariant、long-term solution 是統一 RunId source of truth、由 caller 派生並下傳 verb 而非 verb 自己派生）。驗證：doc 存在、含 issue 描述 + 修法走過的兩個選項 + 為何選 B。
