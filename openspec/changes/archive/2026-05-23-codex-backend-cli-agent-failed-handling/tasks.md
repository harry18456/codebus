<!--
Each task description states the behavior delivered AND the verification target.
File paths appear only as supporting locator context, never as the task itself.
-->

## 1. 補 enum-level test 覆蓋（Verb Error Enum 既有測試擴增）

- [x] 1.1 為 `Verb Error Enum` 要求的 `cli_exit_code` 映射補 AgentFailed 案例：在 `codebus-core/src/verb/error.rs` 既有 `cli_exit_code_mapping_covers_every_variant` test 的 `cases` 陣列加入 `(VerbError::AgentFailed { exit_code: Some(2) }, 1)` 條目。完成行為：test 涵蓋 spec 列出的全部 7 個 variant 之 exit code 映射（既有 6 + AgentFailed 新增 1）。驗證方式：`cargo test -p codebus-core verb::error::tests::cli_exit_code_mapping_covers_every_variant` 通過。
- [x] 1.2 為 `Verb Error Enum` 要求的 AgentFailed Display 條件展開新增 test：新增函式 `agent_failed_display_includes_exit_code`，分別驗 `VerbError::AgentFailed { exit_code: Some(42) }.to_string()` 含子字串 `"42"` 與 `"non-zero status"`、`VerbError::AgentFailed { exit_code: None }.to_string()` 不含括號群但仍含 `"non-zero status"`（per spec §Verb Error Enum example table）。完成行為：Display 條件展開行為被 assertion 鎖定。驗證方式：`cargo test -p codebus-core verb::error::tests::agent_failed_display_includes_exit_code` 通過。

## 2. 補 CLI thin wrapper match arm（解 build break）

- [x] 2.1 在 `codebus-cli/src/commands/chat.rs::translate_error` 加 active `VerbError::AgentFailed { exit_code }` arm 實現 `Verb Error Enum` 要求對 chat path 的 user-facing UX：印 `eprintln!("error: chat: agent exited with code {}", ...)`（`Some(n)` 印數字、`None` 印 `"without a recorded exit code"`），回 `ExitCode::from(err.cli_exit_code())`（即 1）。完成行為：chat CLI 收到 `AgentFailed` 時 stderr 有可讀訊息含 child exit code、CLI 退出 1。驗證方式：`cargo build -p codebus-cli` 對 chat.rs 不再報 E0004；手動 grep 確認 arm 存在且使用 `cli_exit_code()`。
- [x] 2.2 在 `codebus-cli/src/commands/fix.rs`、`goal.rs`、`query.rs`、`quiz.rs` 各自 `translate_error`（或對等 error 翻譯位置）加 defensive `VerbError::AgentFailed { exit_code }` arm 實現 `Verb Error Enum` 要求對 one-shot verb 的 defensive fallback：印 `eprintln!("error: <verb>: agent exited with code {}", ...)`，回 `ExitCode::from(err.cli_exit_code())`（即 1）。**不使用 `unreachable!()`**（per spec：避免未來 regression 觸發 panic）。完成行為：4 個 thin wrapper 對 `AgentFailed` 有 generic fallback、`cargo build -p codebus-cli` 不再報 E0004。驗證方式：`cargo build --workspace` 通過；手動 grep 確認 4 個檔案都有對應 arm 且無 `unreachable!()`。

## 3. 整合驗證

- [x] 3.1 全 codebus-cli build：`cargo build -p codebus-cli` 0 errors 0 warnings（warnings 預設不視為 error 但應留意是否有相關訊息）。完成行為：先前 5 個 E0004 全消失。驗證方式：CLI 輸出。
- [x] 3.2 全工作區 build：`cargo build --workspace` 通過，下游 crate（codebus-app tauri 部分等）亦不受影響。完成行為：整個 workspace 編譯綠。驗證方式：CLI 輸出 + 沒新增 warning。
- [x] 3.3 enum test 覆蓋：`cargo test -p codebus-core verb::error` 既有 6 條 + 新增 2 條共 8 條全綠。完成行為：所有 `Verb Error Enum` 要求對應 test 通過。驗證方式：CLI 輸出顯示 8 passed。
- [x] 3.4 CLI 端 test：`cargo test -p codebus-cli` 全綠（不新增 hook 端測試但既有測試必須通過編譯後 pass）。完成行為：CLI side test suite 通過。驗證方式：CLI 輸出 0 failures。
- [x] 3.5 spectra validate：`spectra validate codex-backend-cli-agent-failed-handling` 通過（spec/tasks 一致性、無 forbidden words、Scenario 格式皆正確）。完成行為：validate 0 errors 0 warnings。驗證方式：CLI 輸出。
- [x] 3.6 手動 sanity（chat path）：build 完後執行 `codebus chat`，挑一個會讓 agent 失敗的場景（e.g. codex `exec resume` 跨 provider 切換）製造非零 exit、確認 stderr 出現「error: chat: agent exited with code <N>」且 exit code 為 1。完成行為：chat path AgentFailed UX 在真實 binary 下可觀察。驗證方式：終端 paste 結果到 PR 描述。
