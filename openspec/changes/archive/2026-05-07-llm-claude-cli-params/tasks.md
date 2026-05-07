## 1. ProviderConfig::ClaudeCli 加 model + effort 欄位

- [x] 1.1 在 codebus-core/src/llm/factory.rs 的 `ProviderConfig::ClaudeCli` variant 加 `model: Option<String>` 與 `effort: Option<String>` 兩個欄位（皆 `#[serde(default, skip_serializing_if = "Option::is_none")]`）；`build_provider` 對 ClaudeCli arm 內 `with_binary` 行為不變、新欄位先單純解構但暫不傳遞（傳遞由 task 3.1 處理）；先寫單元測試覆蓋「model + effort 從 YAML 解析回 Some/None」與「Default 仍是 ClaudeCli { binary_path: None, model: None, effort: None }」再實作

## 2. InvokeOptions 擴展 model + effort

- [x] 2.1 在 codebus-core/src/llm/provider.rs 的 `InvokeOptions` struct 加 `model: Option<String>` 與 `effort: Option<String>` 兩個欄位；對應更新既有 `invoke_options_struct_shape_unchanged_after_lint_feedback_loop` lock-in test 的解構 pattern + 重命名為 `invoke_options_struct_shape_carries_model_and_effort`（這次 trait 介面演進是有意的、跟 lint-feedback-loop 不再綁定）；先寫單元測試覆蓋新欄位 default 為 None 再實作

## 3. build_argv 接受 model + effort 並注入旗標

- [x] 3.1 在 codebus-core/src/llm/providers/claude_cli.rs 的 `build_argv` 函數簽名加 `model: Option<&str>` 與 `effort: Option<&str>` 兩個引數（接 `&str` 避免 caller 重複 clone），當 Some 時於回傳 argv vector 末尾追加 `"--model".into(), v.into()` 與 `"--effort".into(), v.into()`；對 spec scenario「Model flag is injected when ClaudeCli config sets model」、「Effort flag is injected when ClaudeCli config sets effort」、「Model and effort flags are absent when config leaves them None」三條各寫一個單元測試。實作 spec requirement「Spawn LLM agent with sandbox flags and cwd isolation」中關於 model/effort 旗標注入的部分（同時也是 wiki-query「Spawn agent in query mode with Write/Edit excluded from toolset」的對應行為，因兩個 mode 共用 build_argv）

## 4. forbidden flags 架構保險絲

- [x] 4.1 在 codebus-core/src/llm/providers/claude_cli.rs 加 `pub const FORBIDDEN_FLAGS: &[&str] = &["--add-dir", "--allow-dangerously-skip-permissions", "--dangerously-skip-permissions"]`；補一個 negative-assertion 測試針對 `[Ingest, Query]` × `[(model: None, effort: None), (model: Some("sonnet"), effort: None), (model: None, effort: Some("high")), (model: Some("opus"), effort: Some("xhigh"))]` 共 8 種 build_argv 組合，斷言 argv 任何字串都不含 FORBIDDEN_FLAGS 的任何一個，實作 spec requirement 的「Forbidden sandbox-breaking flags never appear in argv」scenario（同時涵蓋 wiki-ingest 跟 wiki-query 對應的 forbidden-flag scenarios）

## 5. ClaudeCliProvider::invoke 把 model + effort 從 InvokeOptions 傳到 build_argv

- [x] 5.1 在 codebus-core/src/llm/providers/claude_cli.rs 的 `ClaudeCliProvider::invoke` 內，把 `opts.model.as_deref()` 與 `opts.effort.as_deref()` 傳給 `build_argv`；既有 build_argv caller 在所有測試模組裡的呼叫方式對應更新（多兩個 None 引數）

## 6. loader 解析 model + effort

- [x] 6.1 在 codebus-core/src/config/loader.rs 的 `parse_llm` 函數內，針對 `claude_cli` variant 構造路徑加 `model: parse_string_or_warn("llm.model", val)` 與 `effort: parse_string_or_warn("llm.effort", val)` 兩條 sub-field 解析（沿用既有 `binary_path` 的 `match val.as_str()` + `warn_type_mismatch` 模式，保持 field-level 容錯）；補單元測試覆蓋 spec scenario「ClaudeCli model and effort are parsed when present」與「ClaudeCli model and effort default to None when absent」。實作 spec requirement「Load global config tolerantly」對 ClaudeCli variant 新欄位的解析行為

## 7. CLI commands 透過 InvokeOptions 傳 model + effort

- [x] 7.1 在 codebus-cli/src/commands/goal.rs / fix.rs / query.rs 三條 build_invoke 路徑：從 `ProviderConfig::ClaudeCli { model, effort, .. }` 抽出 model 跟 effort，塞進對應的 `InvokeOptions { ..., model, effort, ... }`。其他 provider variant（AnthropicApi / Openai / OllamaLocal）目前都未實作 invoke、本 change 不處理；用 match 抽欄位時對非 ClaudeCli variant 都傳 None。涉及 main.rs 從 GlobalConfig 拿 ProviderConfig 的解構部分若需要也對應更新

## 8. 整合測試 + 驗收

- [x] 8.1 codebus-core / codebus-cli 既有測試（含 InvokeOptions 解構的 lock-in test、CollectingRenderer 測試、command integration tests）對應更新後 cargo test --workspace 全綠 + cargo clippy --workspace -- -D warnings 無警告
- [x] 8.2 `~/.codebus/config.yaml` 寫入 `llm: { provider: claude_cli, model: haiku }`，cargo run --release 對 D:/side_project/uv 跑一次小 query，肉眼驗 stderr 或 debug 模式下確實有 `--model haiku` 出現在 spawn argv（或透過 claude CLI 自身的 model 行為間接觀察成本），確認設定真的有效
- [x] 8.3 跑 spectra-audit：審 (a) model + effort 字串未經驗證直接 forward 是否安全（用戶若塞惡意字串如 `--add-dir=/etc` 會被 Claude CLI argv parser 當值看不會當 flag，但仍建議加註說明）；(b) FORBIDDEN_FLAGS 的 scenario 覆蓋是否真把所有 build_argv code path 都打到（包含未來新增 mode 時容易漏的場景）
