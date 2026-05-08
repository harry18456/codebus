## Summary

把 codebus-core 四個 plugin factory 的 flat config struct 全部轉成 serde-tagged enum，讓每個 plugin variant 由 type system 強制只能配置自己合法的欄位。

## Motivation

四個 plugin factory（`llm` / `pii` / `log` / `render`）目前都用同一個「`Kind` enum 當 discriminator + 共用 flat `Config` struct」的形態，把 variant-specific 欄位攤平擺進共用 struct。各 factory 現況：

- **llm/factory.rs `ProviderConfig`**：`binary_path` (ClaudeCli only) / `timeout_secs` (HTTP only) / `api_key` (HTTP only) — **3/4 欄位 variant-specific**
- **pii/factory.rs `ScannerConfig`**：`patterns_extra` 概念上 RegexBasic 限定，未來 Presidio / Aws 還會帶 `presidio_url` / `aws_region` / `aws_profile`
- **log/factory.rs `SinkConfig`**：`jsonl_dir` (Jsonl only)；未來 Otel 會帶 `endpoint` / `auth` / `headers`
- **render/factory.rs `RendererConfig`**：`terminal: RenderOptions` (Terminal only)；未來 Tauri renderer 會帶 channel 名稱 / IPC 設定

這個 flat 設計的隱性假設是「variant-specific 欄位不會多」。實際 roadmap：

- **#1 token tracking** 會把 Otel sink 帶進 log/factory.rs
- **#2 multi-LLM + tool abstraction** 會把 AnthropicApi / OpenAi / OllamaLocal 從 stub 變成真實作，各帶自己的 endpoint / model-list / auth 欄位
- **Tauri tutorial app** 會把 Tauri renderer 帶進 render/factory.rs
- **Heavy-dep PII scanners** 會把 Presidio / Aws 從 stub 變實作

四個 factory 都會在未來 motivating feature 階段觸發 variant-specific 欄位增加。**現在不 refactor，每個 motivating feature 都要先做一次 retrofit 再加自己的欄位**，多次修改 YAML schema 對用戶體感更差。

把這個結構性 refactor 提前到一個 standalone change 做，後續每個 motivating feature change 落地時都是「在對應 variant 內加欄位」純加法，不再動底層 schema。

## Proposed Solution

**1. 四個 factory config 全部從 struct 變 serde-tagged enum**：

```rust
// llm/factory.rs
pub enum ProviderConfig {
    ClaudeCli { binary_path: Option<String> },
    AnthropicApi { api_key: Option<String>, timeout_secs: Option<u64> },
    OpenAi { api_key: Option<String>, timeout_secs: Option<u64> },
    OllamaLocal {},
}
```

```rust
// pii/factory.rs
pub enum ScannerConfig {
    Null { on_hit: OnHit },
    RegexBasic { on_hit: OnHit, patterns_extra: Vec<String> },
    Presidio { on_hit: OnHit },
    Aws { on_hit: OnHit },
}
```

```rust
// log/factory.rs
pub enum SinkConfig {
    Null {},
    Jsonl { dir: Option<PathBuf>, retention_days: Option<u32> },
    Otel {},
}
```

```rust
// render/factory.rs
pub enum RendererConfig {
    Terminal { options: RenderOptions },
    JsonLines {},
    Tauri {},
}
```

每個 enum 用 `#[serde(tag = "...")]` 配既有 discriminator 名（`provider` / `scanner` / `sink` / `format`），保持既有 YAML key 不變。

**2. config/schema.rs 對應更新**：四個 `LlmConfig` / `PiiConfig` / `LogConfig` / `RenderConfig` 也轉 tagged enum，作為 `GlobalConfig` 各 section 的型別。

**3. loader.rs 維持 tolerant 行為**：unknown discriminator value（如 `provider: gibberish`）→ catch serde error、warn 進 stderr、退回 default。實作上用 `Option<EnumType>` + 對 None 給 default 的策略，包住 `serde_yaml::from_value`。

**4. 各 factory 的 build 函數**：match arm 對應 variant 結構解構，行為對等。

**5. main.rs mapping 函數**：`provider_config_from` / `scanner_config_from` / `sink_config_from` / `renderer_config_from` 對應改寫，從 GlobalConfig 各 section variant 抽出對應欄位餵給對應 factory。

**6. design.md 寫入 「Tagged-enum config pattern」段**：給未來其他 plugin factory 加 variant 用的範本，避免再 retrofit。

## Non-Goals

- **不加任何新欄位**：`model` / `effort` / `fallback_model` / `max_budget_usd` / endpoint / region / 等等留給後續 motivating feature change（本 change 完成後 ClaudeCli params 那條 follow-up 接著做）
- **不改變任何 plugin runtime 行為**：純 shape refactor，所有 build_* 函數的 input/output 對等
- **不重命名既有欄位**：YAML key 維持不變
- **不擴 LlmProvider / PiiScanner / LogSink / EventRenderer trait surface**：trait 方法簽名不動
- **不動 LintConfig / AutoFixConfig / GlobalConfig / EmojiMode**：這些不是 factory pattern，沒有 variant 概念
- **不為 LLM 加 ProposalEmitted / gap_detection 等 query-gap 功能**：那是 #2 multi-LLM + tool abstraction 階段的工作
- **不擴 RegexBasic/Jsonl 之類已有 variant 的欄位**：保持 1:1 從 flat 移到 variant，不順便加

## Alternatives Considered

**(a) 維持 flat struct，等個別 motivating feature 再各自 refactor**：每個 motivating feature change（token tracking / multi-LLM / Tauri / Presidio）內含一段 factory refactor + 加欄位，混合 structural change 跟 feature change。reviewer 難切分，每次都要動 YAML schema，用戶 config.yaml 多次遷移。否決。

**(b) 用「flat struct + variant-specific sub-struct」的中繼形態**（例：`ProviderConfig.claude_cli: ClaudeCliSettings`）：保留 flat top-level，把 variant-specific 欄位包進子 struct。比 (a) 乾淨一點，但沒拿到 type system 強制（用戶可以同時設 `provider: claude_cli` 跟 `openai_specific.organization: ...`，邏輯矛盾不會被 schema 攔），且未來仍要再 refactor 一次到真 tagged enum。否決。

**(c) 只 refactor llm/factory.rs，其他三個等個別 motivating feature 再做**：llm 確實是最急（馬上要加 model / effort），但選 (c) 會讓四個 factory 在中期處於「pattern 不一致」狀態，contributor 看到 llm 是 tagged enum、其他三個是 flat 會困惑「該照哪個寫」。一致性的 documentation cost 比一次 refactor 高。否決。

**選擇本提案（一口氣 refactor 四個）**：所有 factory pattern 一致，未來各 motivating feature 都是純加法。一次 BREAKING（影響很小，因為 codebus 用戶基數還小）換長期穩定。

## Impact

- Affected specs:
  - `terminal-output`（MODIFIED）：「Load global config tolerantly」requirement 描述微幅 refresh，明確各 plugin section 的 discriminator 與 variant-specific 欄位由 type 強制；scenarios 大部分原文保留（user-observable 行為對等），新增一個 scenario 涵蓋「variant-mismatched 欄位被 serde 預設 ignore」
- Affected code:
  - Modified: codebus-core/src/llm/factory.rs（ProviderConfig 轉 tagged enum、build_provider match 重寫）
  - Modified: codebus-core/src/pii/factory.rs（ScannerConfig 轉 tagged enum、build_scanner match 重寫）
  - Modified: codebus-core/src/log/factory.rs（SinkConfig 轉 tagged enum、build_sink match 重寫）
  - Modified: codebus-core/src/render/factory.rs（RendererConfig 轉 tagged enum、build_renderer match 重寫）
  - Modified: codebus-core/src/config/schema.rs（LlmConfig / PiiConfig / LogConfig / RenderConfig 轉 tagged enum）
  - Modified: codebus-core/src/config/loader.rs（unknown discriminator → warn + default 的 fallback 邏輯改用 catch error 包住 serde_yaml::from_value）
  - Modified: codebus-core/src/llm/mod.rs（re-export 對應 enum 而非舊 struct，如有需要）
  - Modified: codebus-core/src/pii/mod.rs（同上）
  - Modified: codebus-core/src/log/mod.rs（同上）
  - Modified: codebus-core/src/render/mod.rs（同上）
  - Modified: codebus-cli/src/main.rs（provider_config_from / scanner_config_from / sink_config_from / renderer_config_from 重寫，對應 GlobalConfig 各 section variant）
  - Modified: codebus-cli/tests/config_integration.rs（既有 YAML round-trip 測試對應更新到 tagged enum 形態）
- Affected dependencies: 無新增
