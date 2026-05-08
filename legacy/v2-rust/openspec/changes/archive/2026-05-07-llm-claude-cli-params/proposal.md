## Why

`ProviderConfig::ClaudeCli` 目前只接 `binary_path`，沒有讓用戶選 LLM model 或 reasoning effort 的途徑。實際使用上：

- **成本控制**：query mode 用 haiku 比 sonnet 便宜約 80%，但用戶現在沒法切。每次 query 都吃 sonnet 是浪費
- **品質彈性**：複雜 goal 想用 opus 跑、簡單探索用 sonnet 即可，今天全部一律走 Claude CLI 預設值
- **效率調整**：reasoning-heavy 任務（重構分析、複雜結構梳理）受惠於 high/xhigh effort，輕量 query 用 low 就夠

Claude CLI 早就支援 `--model` 跟 `--effort` 兩個旗標，codebus 只是還沒把它們接出來。剛 archive 的 `config-tagged-enum-refactor` 把 `ProviderConfig` 改成 tagged enum 就是為了讓本 change 變成「在 ClaudeCli variant 加兩個欄位」純加法 —— 沒有底層 schema 改動。

## What Changes

- `ProviderConfig::ClaudeCli` variant 加兩個 optional 欄位：`model: Option<String>` 與 `effort: Option<String>`
- `build_argv` 簽名擴展，接受 `model` 與 `effort` 引數；當 `Some` 時於 argv 注入 `--model <m>` / `--effort <level>` 旗標，當 `None` 時不注入（沿用 Claude CLI 預設值，保留 0.2.0 行為對等）
- `loader::parse_llm` 在既有 `binary_path` 解析旁邊加 `model` / `effort` 兩條 sub-field 解析，沿用 field-level 容錯（type-mismatched 只該欄位 None）
- `commands/goal.rs` / `commands/fix.rs` / `commands/query.rs` 將從 `ProviderConfig::ClaudeCli` 結構傳出的 model + effort 一路接到 `build_argv` 呼叫
- 新增 negative-assertion test：`--add-dir` / `--allow-dangerously-skip-permissions` / `--dangerously-skip-permissions` 絕不出現在任何 mode + 任何 model/effort 組合下的 argv（架構保險絲，pin 住 sandbox 紅線）

## Non-Goals

- **不加 `fallback_model`**：Claude CLI 雖有 `--fallback-model` 旗標，但若要做 cross-provider 通用版必須由 codebus 自己 client-side 重試。等 #1 token tracking 落地、有 cost 資料 + 重試框架後再加。
- **不加 `max_budget_usd`**：同理，Claude CLI 雖有 `--max-budget-usd`，但要 cross-provider 通用必須 codebus 自己累加成本 + 比對價目表。等 #1 token tracking 一起做。
- **不加 Tier B 旗標**：`--bare` / `--debug-file` / `--betas` / `--include-partial-messages` 沒迫切需求，未來各自有 motivating feature 再加。
- **不動其他 provider variant**：`AnthropicApi` / `Openai` / `OllamaLocal` 雖然各家也有 model 概念但 wire format 不同（OpenAI 用 model 字串、Anthropic API 用 enum、Ollama 用 model file 路徑），各自實作留給 #2 multi-LLM。
- **不引入 per-mode model**：譬如「query 用 haiku、goal 用 opus」的彈性。先走全域單一 model，等 #1 token tracking 觀察實際 cost / 品質權衡後再決定要不要 per-mode override。
- **不驗證 model / effort 字串值**：codebus 不知道 Claude CLI 認不認某個 model 名稱（model 別名會跟新版 CLI 一起更新），把字串直接 forward 給 Claude CLI 自己驗。Tier D 紅線旗標例外（必須擋住）。

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `terminal-output`: 既有「Load global config tolerantly」requirement 新增 scenario 涵蓋「YAML 內含 model + effort 時被 loader 正確解析進 ClaudeCli variant」。
- `wiki-ingest`: 既有「Spawn LLM agent with sandbox flags and cwd isolation」requirement 新增兩個 scenario：「model 與 effort 在 Some 時注入 --model / --effort 旗標、None 時不注入」與「Tier D 紅線旗標 (`--add-dir` / `--allow-dangerously-skip-permissions` / `--dangerously-skip-permissions`) 在任何 model/effort 組合下絕不出現於 argv」。
- `wiki-query`: 同上的兩個 scenario，套用在 query mode argv 上。

## Impact

- Affected specs: `terminal-output`（MODIFIED）、`wiki-ingest`（MODIFIED）、`wiki-query`（MODIFIED）
- Affected code:
  - Modified: codebus-core/src/llm/factory.rs（ClaudeCli variant 加 `model` 與 `effort` 欄位）
  - Modified: codebus-core/src/llm/providers/claude_cli.rs（`build_argv` 簽名擴充、`--model` / `--effort` 旗標注入、`ClaudeCliProvider::invoke` 從 `InvokeOptions` 取得 model + effort 傳給 build_argv；`FORBIDDEN_FLAGS` constant + negative-assertion test）
  - Modified: codebus-core/src/llm/provider.rs（`InvokeOptions` 加 `model: Option<String>` 與 `effort: Option<String>` 兩個欄位；既有 `invoke_options_struct_shape_unchanged_after_lint_feedback_loop` lock-in test 解構需要對應更新，這是有意的設計演進）
  - Modified: codebus-core/src/config/loader.rs（`parse_llm` 在 ClaudeCli variant 內加 `model` 與 `effort` 兩條 sub-field 解析）
  - Modified: codebus-cli/src/commands/goal.rs（從 `ProviderConfig::ClaudeCli` 抽出 model + effort 並透過 `InvokeOptions` 傳遞）
  - Modified: codebus-cli/src/commands/fix.rs（同上）
  - Modified: codebus-cli/src/commands/query.rs（同上）
- Affected dependencies: 無
