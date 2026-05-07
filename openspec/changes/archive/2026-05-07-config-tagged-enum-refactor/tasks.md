## 1. llm factory tagged enum

- [x] 1.1 [P] 把 codebus-core/src/llm/factory.rs 的 `ProviderConfig` 改成 `#[serde(tag = "provider", rename_all = "snake_case")]` tagged enum（`ClaudeCli` / `AnthropicApi` / `OpenAi` / `OllamaLocal` 四 variant，現有 flat 欄位按目前對應分配進 variant 內），加 `impl Default` 回 `ClaudeCli { binary_path: None }`；改寫 `build_provider` 為對 enum 直接 match 而不是讀 `Kind` discriminator；先寫每 variant 的 deserialize / build / default 單元測試再實作。實作 design 決策 "Tagged-enum config pattern" 與 "Default variant 的選擇"

## 2. pii factory tagged enum

- [x] 2.1 [P] 把 codebus-core/src/pii/factory.rs 的 `ScannerConfig` 改成 `#[serde(tag = "scanner", rename_all = "snake_case")]` tagged enum（`Null` / `RegexBasic` / `Presidio` / `Aws` 四 variant），`on_hit` 在每個 variant 內、`patterns_extra` 只收進 `RegexBasic` variant，加 `impl Default` 回 `Null { on_hit: OnHit::Warn }`；改寫 `build_scanner` 為對 enum match；先寫測試再實作。實作 design 決策 "`on_hit` 在 ScannerConfig 內的定位"

## 3. log factory tagged enum

- [x] 3.1 [P] 把 codebus-core/src/log/factory.rs 的 `SinkConfig` 改成 `#[serde(tag = "sink", rename_all = "snake_case")]` tagged enum（`Null` / `Jsonl` / `Otel` 三 variant），`jsonl_dir` 跟 `retention_days` 兩個都收進 `Jsonl` variant（`Jsonl { dir: Option<PathBuf>, retention_days: Option<u32> }`），加 `impl Default` 回 `Null {}`；改寫 `build_sink` 為對 enum match；先寫測試再實作

## 4. render factory tagged enum

- [x] 4.1 [P] 把 codebus-core/src/render/factory.rs 的 `RendererConfig` 改成 `#[serde(tag = "format", rename_all = "snake_case")]` tagged enum（`Terminal` / `JsonLines` / `Tauri` 三 variant），`RenderOptions` 收進 `Terminal { options: RenderOptions }`，加 `impl Default` 回 `Terminal { options: Default::default() }`；改寫 `build_renderer` 為對 enum match；先寫測試再實作

## 5. Loader 改輸出 tagged enum（保留 field-level 容錯）

- [x] 5.1 改寫 codebus-core/src/config/loader.rs 的 `parse_llm` / `parse_pii` / `parse_log` / `parse_render` 四個函數：保留現有手動 `serde_yaml::Value` 走訪結構（per-field try-parse + per-field warn）、輸出型別從中繼 flat struct 改成 factory 的 tagged enum；unknown discriminator 仍走「warn + return None（整段退 default）」、type-incompatible sub-field 仍走「warn + 該欄位 None（其他保留）」、missing discriminator 走預設 variant；先確保所有現有 loader 測試（含 `type_mismatched_sub_field_is_treated_as_unset`、`unknown_llm_provider_treats_section_as_unset`、`empty_plugin_section_parses_as_defaults` 等）在新輸出型別下重寫並通過。實作 design 決策 "Loader 容錯行為的整合方式"

## 6. config/schema.rs 清理 — GlobalConfig 直接持有 tagged enum

- [x] 6.1 把 codebus-core/src/config/schema.rs 的中繼 flat struct（`LlmConfig` / `PiiConfig` / `LogConfig` / `RenderConfig`）連同 `#[serde(skip)]` discriminator hack 整批移除；`GlobalConfig` 各對應 plugin section 欄位型別改成直接持有 factory 的 tagged enum（如 `pub llm: Option<crate::llm::ProviderConfig>`）；同步更新各 plugin domain 的 `mod.rs` re-export 表面（移除已不存在的 *Config 型別、新增 enum re-export 如尚未 export）。先補單元測試覆蓋 modified spec requirement "Load global config tolerantly" 的所有 scenarios，特別包括新增的「Sub-field valid in a sibling variant is silently ignored」case

## 7. main.rs mapper 簡化

- [x] 7.1 改寫 codebus-cli/src/main.rs 的 `provider_config_from` / `scanner_config_from` 等函數，輸入是 `&GlobalConfig`、輸出是各對應 tagged enum；現在 GlobalConfig 已直接持有 tagged enum，mapper 退化為「拿到 None section → 回 enum 的 Default、拿到 Some(variant) → clone 後直接傳給 factory」一行邏輯。如某個 *_config_from 目前不存在（譬如 sink/renderer 主流程沒在用），仍把該函數補齊保持四個 mapper 對稱。既有 caller 簽名不變

## 8. 整合測試 + 驗收

- [x] 8.1 codebus-cli/tests/config_integration.rs 既有 YAML round-trip 測試對應更新到 tagged enum 形態（YAML 文字其實不變，但解析後產出的 Rust struct 形態改了，assertion 對應 variant pattern matching）
- [x] 8.2 cargo test --workspace 全綠 + cargo clippy --workspace -- -D warnings 無警告；確認 256+ 既有測試在 refactor 後 0 regression（特別是 loader.rs 的 field-level 容錯測試）
- [x] 8.3 跑 spectra-audit：審 ProviderConfig / ScannerConfig / SinkConfig / RendererConfig 各自的 Default impl 是否真對齊 0.2.0 plugin 行為（loader 給 None section 與用戶寫 `<section>: {}` 兩種 case 產出的 plugin 行為應與 refactor 前對等），以及 loader 的 field-level 容錯（type-incompatible sub-field 該失敗那一欄、其他保留）在新 tagged enum 輸出下確實維持
